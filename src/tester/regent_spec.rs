use anyhow::{Context, Result};
use regex::{Regex, RegexBuilder};
use serde::Deserialize;
use serde_json::Value as JsonValue;
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use super::{CoverageReport, FileCoverage, TestCase, TestResults, TestStatus};
use crate::tester::puppet_eval::{
    normalize_rtype, EvaluationTrace, PuppetCatalog, PuppetEvaluator, PuppetValue,
};

#[derive(Debug, Deserialize)]
pub struct RegentPlan {
    pub tests: Vec<RegentTest>,
    /// Spec files that raised while being loaded (unsupported helper, missing
    /// constant, …). Each becomes one failed example so the rest of the suite
    /// still reports real results instead of the whole run aborting.
    #[serde(default)]
    pub load_errors: Vec<LoadError>,
}

#[derive(Debug, Deserialize)]
pub struct LoadError {
    pub file: String,
    pub error: String,
    #[serde(default)]
    pub backtrace: String,
}

#[derive(Debug, Deserialize)]
pub struct RegentTest {
    pub name: String,
    pub subject: String,
    pub title: Option<String>,
    /// `let(:node) { 'foo.example.com' }`. Real Puppet/PDK derives the
    /// `fqdn`/`hostname`/`domain` facts from this; we do the same so manifests
    /// that default a parameter to `$fqdn` resolve instead of rendering undef.
    #[serde(default)]
    pub node: Option<String>,
    pub facts: Option<HashMap<String, JsonValue>>,
    pub params: Option<HashMap<String, JsonValue>>,
    pub expectations: Vec<Expectation>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind")]
pub enum Expectation {
    #[serde(rename = "compile")]
    Compile {
        #[serde(default)]
        negate: bool,
        /// `compile.with_all_deps` — additionally assert that every relationship
        /// reference in the catalog resolves to a declared resource.
        #[serde(default)]
        check_all_deps: bool,
    },
    #[serde(rename = "contain")]
    Contain {
        resource_type: String,
        title: String,
        #[serde(default)]
        attributes: HashMap<String, JsonValue>,
        /// `that_requires` / `that_comes_before` / `that_notifies` /
        /// `that_subscribes_to` references attached to this resource.
        #[serde(default)]
        relationships: Vec<Relationship>,
        #[serde(default)]
        negate: bool,
    },
    /// `is_expected.to compile.and_raise_error(/msg/)` or
    /// `expect { ... }.to raise_error(Klass, /msg/)`. Both forms assert that
    /// compiling the subject fails; an optional `message` (regex or substring)
    /// constrains the error text. The exception class, if given in the spec,
    /// is ignored — the evaluator surfaces a single generic compile error.
    #[serde(rename = "raise_error")]
    RaiseError {
        #[serde(default)]
        message: Option<JsonValue>,
        #[serde(default)]
        negate: bool,
    },
    /// `is_expected.to have_<type>_resource_count(n)` (or the typeless
    /// `have_resource_count(n)`). Asserts the compiled catalog holds exactly
    /// `count` resources of `resource_type` — or, when `resource_type` is
    /// `None`, that many non-class resources in total.
    #[serde(rename = "resource_count")]
    ResourceCount {
        #[serde(default)]
        resource_type: Option<String>,
        count: i64,
        #[serde(default)]
        negate: bool,
    },
    /// `is_expected.to allow_value(v)` on a `describe '<TypeAlias>'` block.
    /// Each value is checked against the described Puppet type alias.
    #[serde(rename = "allow_value")]
    AllowValue {
        #[serde(default)]
        values: Vec<JsonValue>,
        #[serde(default)]
        negate: bool,
    },
    /// An in-Ruby value assertion (`expect(x).to eq(y)`) already evaluated by
    /// the DSL — used by non-catalog specs (e.g. custom-fact unit tests). The
    /// result is carried through verbatim.
    #[serde(rename = "value_assertion")]
    ValueAssertion {
        #[serde(default)]
        passed: bool,
        #[serde(default)]
        message: Option<String>,
    },
}

/// One relationship assertion from a `contain_*` matcher, e.g.
/// `contain_service('a').that_requires('Package[b]')`.
#[derive(Debug, Deserialize)]
pub struct Relationship {
    /// `require`, `before`, `notify`, or `subscribe`.
    pub kind: String,
    /// The target reference text, e.g. `"Package[b]"`.
    pub target: String,
}

/// A `Type[title]` reference, normalized for catalog lookups (type lowercased
/// with `__` → `::`, surrounding quotes/whitespace stripped from the title).
type ResourceRef = (String, String);

/// The catalog's relationship edges, derived from the four relationship
/// metaparameters. Puppet treats them as two ordering pairs plus refresh:
///   require  => X   : X is applied before the declaring resource
///   before   => X   : the declaring resource is applied before X
///   subscribe => X  : X before declarer, and declarer refreshes from X
///   notify   => X   : declarer before X, and X refreshes from declarer
/// `before` holds ordering edges (a applied before b); `notify` holds refresh
/// edges (a notifies b). The inverse forms collapse into the same edges, so a
/// dependency declared from either end satisfies the matcher.
#[derive(Default)]
struct DepGraph {
    before: HashSet<(ResourceRef, ResourceRef)>,
    notify: HashSet<(ResourceRef, ResourceRef)>,
}

/// Parse a `Type[title]` reference. Returns `None` for text that isn't a
/// resource reference (those are simply not relationships).
fn parse_resource_ref(text: &str) -> Option<ResourceRef> {
    let open = text.find('[')?;
    let close = text.rfind(']')?;
    if close <= open {
        return None;
    }
    let rtype = text[..open].trim();
    let title = text[open + 1..close]
        .trim()
        .trim_matches(|c| c == '\'' || c == '"');
    if rtype.is_empty() || title.is_empty() {
        return None;
    }
    Some((normalize_rtype(rtype), title.to_string()))
}

/// Collect every resource reference in a metaparameter value, flattening
/// arrays (`require => [File['a'], Service['b']]`).
fn collect_refs(value: &PuppetValue, out: &mut Vec<ResourceRef>) {
    match value {
        PuppetValue::String(text) => {
            if let Some(reference) = parse_resource_ref(text) {
                out.push(reference);
            }
        }
        PuppetValue::Array(items) => {
            for item in items {
                collect_refs(item, out);
            }
        }
        _ => {}
    }
}

const RELATIONSHIP_METAPARAMS: [&str; 4] = ["require", "before", "notify", "subscribe"];

fn build_dep_graph(catalog: &PuppetCatalog) -> DepGraph {
    let mut graph = DepGraph::default();
    for resource in catalog.iter_resources() {
        let here: ResourceRef = (resource.resource_type.clone(), resource.title.clone());
        for meta in RELATIONSHIP_METAPARAMS {
            let Some(value) = resource.attributes.get(meta) else {
                continue;
            };
            let mut refs = Vec::new();
            collect_refs(value, &mut refs);
            for target in refs {
                match meta {
                    "require" => {
                        graph.before.insert((target, here.clone()));
                    }
                    "before" => {
                        graph.before.insert((here.clone(), target));
                    }
                    "subscribe" => {
                        graph.before.insert((target.clone(), here.clone()));
                        graph.notify.insert((target, here.clone()));
                    }
                    "notify" => {
                        graph.before.insert((here.clone(), target.clone()));
                        graph.notify.insert((here.clone(), target));
                    }
                    _ => {}
                }
            }
        }
    }
    graph
}

pub struct RegentSpecRunner {
    evaluator: PuppetEvaluator,
}

impl RegentSpecRunner {
    pub fn new(module_path: &Path) -> Result<Self> {
        let evaluator = PuppetEvaluator::new(module_path)?;
        Ok(Self { evaluator })
    }

    pub fn run_plan(&self, plan: RegentPlan) -> Result<TestResults> {
        let mut results = TestResults::new("unit");
        let mut failure_lines = Vec::new();
        let mut covered_classes: HashSet<String> = HashSet::new();
        let mut covered_defines: HashSet<String> = HashSet::new();
        for load_error in &plan.load_errors {
            let name = format!("{} (failed to load)", load_error.file);
            failure_lines.push(format!("{}: {}", name, load_error.error));
            results.add_test_case(TestCase {
                name,
                status: TestStatus::Failed,
                duration_ms: 0,
                message: Some(load_error.error.clone()),
            });
        }
        for test in plan.tests {
            let case_result = self.run_test(&test, &mut covered_classes, &mut covered_defines)?;
            if case_result.status == TestStatus::Failed {
                if let Some(message) = &case_result.message {
                    failure_lines.push(format!("{}: {}", case_result.name, message));
                } else {
                    failure_lines.push(case_result.name.clone());
                }
            }
            results.add_test_case(case_result);
        }
        results.exit_code = if results.failed > 0 { 1 } else { 0 };
        if !failure_lines.is_empty() {
            results.stderr = failure_lines.join("\n");
        }
        results.coverage = Some(self.build_coverage(&covered_classes, &covered_defines));
        Ok(results)
    }

    fn build_coverage(
        &self,
        covered_classes: &HashSet<String>,
        covered_defines: &HashSet<String>,
    ) -> CoverageReport {
        use std::path::PathBuf;
        // Map every primary-module .pp file to "touched?" by checking whether
        // any evaluated class or define originated in it.
        let mut touched: HashSet<PathBuf> = HashSet::new();
        for name in covered_classes {
            if let Some(path) = self.evaluator.class_origin_file(name) {
                touched.insert(path.to_path_buf());
            }
        }
        for name in covered_defines {
            if let Some(path) = self.evaluator.define_origin_file(name) {
                touched.insert(path.to_path_buf());
            }
        }

        let mut file_coverage: HashMap<String, FileCoverage> = HashMap::new();
        let mut lines_total = 0usize;
        let mut lines_covered = 0usize;
        for path in self.evaluator.primary_manifest_files() {
            let display = path.display().to_string();
            let total = std::fs::read_to_string(path)
                .map(|s| s.lines().count().max(1))
                .unwrap_or(1);
            let is_touched = touched.contains(path);
            let covered = if is_touched { total } else { 0 };
            lines_total += total;
            lines_covered += covered;
            let pct = if total == 0 {
                0.0
            } else {
                (covered as f64 / total as f64) * 100.0
            };
            file_coverage.insert(
                display.clone(),
                FileCoverage {
                    path: display,
                    coverage: pct,
                    lines_covered: covered,
                    lines_total: total,
                },
            );
        }
        let overall = if lines_total == 0 {
            0.0
        } else {
            (lines_covered as f64 / lines_total as f64) * 100.0
        };
        CoverageReport {
            overall_coverage: overall,
            lines_covered,
            lines_total,
            branches_covered: 0,
            branches_total: 0,
            file_coverage,
        }
    }

    fn run_test(
        &self,
        test: &RegentTest,
        covered_classes: &mut HashSet<String>,
        covered_defines: &mut HashSet<String>,
    ) -> Result<TestCase> {
        let mut facts = PuppetValue::from_json_map(test.facts.as_ref());
        derive_node_facts(&mut facts, test.node.as_deref());
        let params = PuppetValue::from_json_map(test.params.as_ref());

        let catalog_result = self.evaluate_subject(test, &facts, &params);
        if let Ok((_, trace)) = &catalog_result {
            covered_classes.extend(trace.classes.iter().cloned());
            covered_defines.extend(trace.defines.iter().cloned());
        }
        let catalog_result = catalog_result.map(|(catalog, _)| catalog);

        let mut failures = Vec::new();
        for expectation in &test.expectations {
            match expectation {
                Expectation::Compile {
                    negate,
                    check_all_deps,
                } => match (&catalog_result, *negate) {
                    (Ok(catalog), false) => {
                        if *check_all_deps {
                            if let Err(err) = self.check_all_deps(catalog) {
                                failures.push(err.to_string());
                            }
                        }
                    }
                    (Err(_), true) => {}
                    (Err(err), false) => failures.push(format!("compile failed: {err:#}")),
                    (Ok(_), true) => {
                        failures.push("expected compile to fail, but it succeeded".to_string())
                    }
                },
                Expectation::Contain {
                    resource_type,
                    title,
                    attributes,
                    relationships,
                    negate,
                } => {
                    let Ok(catalog) = &catalog_result else {
                        if !*negate {
                            failures.push(format!(
                                "compile failed: {}",
                                catalog_result.as_ref().err().unwrap()
                            ));
                        }
                        continue;
                    };
                    let check = self
                        .check_resource(catalog, resource_type, title, attributes)
                        .and_then(|()| {
                            self.check_relationships(catalog, resource_type, title, relationships)
                        });
                    match (check, *negate) {
                        (Ok(()), false) | (Err(_), true) => {}
                        (Err(err), false) => failures.push(err.to_string()),
                        (Ok(()), true) => failures.push(format!(
                            "expected catalog NOT to contain {}[{}] (with matching attrs), but it did",
                            resource_type, title
                        )),
                    }
                }
                Expectation::ResourceCount {
                    resource_type,
                    count,
                    negate,
                } => {
                    let Ok(catalog) = &catalog_result else {
                        if !*negate {
                            failures.push(format!(
                                "compile failed: {}",
                                catalog_result.as_ref().err().unwrap()
                            ));
                        }
                        continue;
                    };
                    let actual = match resource_type {
                        Some(rt) => {
                            let rt = normalize_rtype(rt);
                            catalog
                                .iter_resources()
                                .filter(|r| r.resource_type == rt)
                                .count() as i64
                        }
                        // rspec-puppet's bare resource_count excludes the
                        // implicit `class` resources from the total.
                        None => catalog
                            .iter_resources()
                            .filter(|r| r.resource_type != "class")
                            .count() as i64,
                    };
                    let label = resource_type
                        .as_deref()
                        .map(|rt| format!("{rt} "))
                        .unwrap_or_default();
                    match (actual == *count, *negate) {
                        (true, false) | (false, true) => {}
                        (false, false) => failures.push(format!(
                            "expected catalog to have {count} {label}resource(s), but found {actual}"
                        )),
                        (true, true) => failures.push(format!(
                            "expected catalog NOT to have {count} {label}resource(s), but it did"
                        )),
                    }
                }
                Expectation::ValueAssertion { passed, message } => {
                    if !*passed {
                        failures.push(
                            message
                                .clone()
                                .unwrap_or_else(|| "value assertion failed".to_string()),
                        );
                    }
                }
                Expectation::AllowValue { values, negate } => {
                    let type_name = test.subject.trim();
                    for value in values {
                        let pv = PuppetValue::from_json(value);
                        match self.evaluator.type_allows(type_name, &pv) {
                            None => failures.push(format!(
                                "unknown Puppet type {type_name:?} (no matching types/ alias)"
                            )),
                            // For `.to` (negate=false) the value must be allowed;
                            // for `.not_to` (negate=true) it must be rejected.
                            // Failure when allowed == negate.
                            Some(allowed) if allowed == *negate => {
                                let verb = if *negate {
                                    "unexpectedly allows"
                                } else {
                                    "does not allow"
                                };
                                failures.push(format!("{type_name} {verb} value {pv:?}"));
                            }
                            Some(_) => {}
                        }
                    }
                }
                Expectation::RaiseError { message, negate } => {
                    // `{:#}` flattens the whole anyhow context chain so the
                    // message pattern can match text from any wrapping layer.
                    let raised = catalog_result.as_ref().err().map(|err| format!("{err:#}"));
                    if *negate {
                        if let Some(err) = &raised {
                            failures.push(format!(
                                "expected compilation NOT to raise an error, but it raised: {err}"
                            ));
                        }
                    } else {
                        match &raised {
                            None => failures.push(
                                "expected compilation to raise an error, but it succeeded"
                                    .to_string(),
                            ),
                            Some(err) => {
                                if !error_matches(message.as_ref(), err)? {
                                    failures.push(format!(
                                        "expected error to match {}, but got: {err}",
                                        describe_pattern(message.as_ref())
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }

        if test.expectations.is_empty() {
            if let Err(err) = &catalog_result {
                return Ok(TestCase {
                    name: test.name.clone(),
                    status: TestStatus::Failed,
                    duration_ms: 0,
                    message: Some(format!("compile failed: {err:#}")),
                });
            }
        }

        if failures.is_empty() {
            Ok(TestCase {
                name: test.name.clone(),
                status: TestStatus::Passed,
                duration_ms: 0,
                message: None,
            })
        } else {
            Ok(TestCase {
                name: test.name.clone(),
                status: TestStatus::Failed,
                duration_ms: 0,
                message: Some(failures.join("; ")),
            })
        }
    }

    fn evaluate_subject(
        &self,
        test: &RegentTest,
        facts: &PuppetValue,
        params: &PuppetValue,
    ) -> Result<(PuppetCatalog, EvaluationTrace)> {
        let subject = test.subject.trim();
        let title = test.title.as_deref();
        if title.is_some() || self.evaluator.is_define(subject) {
            let title = title.unwrap_or(subject);
            self.evaluator
                .evaluate_define_traced(subject, title, facts, params)
                .with_context(|| format!("define {subject}"))
        } else if self.evaluator.is_class(subject) {
            // The class exists: surface its real compile error verbatim rather
            // than wrapping it in a misleading "not found" context.
            self.evaluator.evaluate_class_traced(subject, facts, params)
        } else {
            let known = self.evaluator.class_names();
            let detail = if known.is_empty() {
                "no classes were loaded (check that manifests/ exists and parses)".to_string()
            } else {
                let preview: Vec<String> = known.iter().take(20).cloned().collect();
                format!(
                    "not found among {} loaded classes (e.g. {})",
                    known.len(),
                    preview.join(", ")
                )
            };
            Err(anyhow::anyhow!("class {subject}: {detail}"))
        }
    }

    fn check_resource(
        &self,
        catalog: &PuppetCatalog,
        resource_type: &str,
        title: &str,
        attributes: &HashMap<String, JsonValue>,
    ) -> Result<()> {
        let resource_type = resource_type.to_lowercase();
        let Some(resource) = catalog.find(&resource_type, title) else {
            let present: Vec<String> = catalog
                .iter_resources()
                .filter(|r| r.resource_type == resource_type)
                .map(|r| r.title.clone())
                .collect();
            return Err(anyhow::anyhow!(
                "missing resource {}[{}] (present {}: {:?})",
                resource_type,
                title,
                resource_type,
                present
            ));
        };
        for (key, value) in attributes {
            let actual = resource
                .attributes
                .get(key)
                .cloned()
                .unwrap_or(PuppetValue::Undef);
            if let Some(pattern) = regex_marker(value) {
                let regex = compile_spec_regex(pattern).map_err(|err| {
                    anyhow::anyhow!(
                        "resource {}[{}] attribute {}: invalid regex {:?}: {}",
                        resource_type,
                        title,
                        key,
                        pattern,
                        err
                    )
                })?;
                if !regex.is_match(&actual.as_string()) {
                    return Err(anyhow::anyhow!(
                        "resource {}[{}] attribute {} expected to match /{}/ but got {:?}",
                        resource_type,
                        title,
                        key,
                        pattern,
                        actual.as_string()
                    ));
                }
                continue;
            }
            let expected = PuppetValue::from_json(value);
            // rspec-puppet compares parameter values loosely across the
            // string/number boundary (a spec asserting `order => '10'` matches a
            // catalog Integer `10`), so fall back to string equality when the
            // structured values differ only by scalar type.
            if expected != actual && expected.as_string() != actual.as_string() {
                return Err(anyhow::anyhow!(
                    "resource {}[{}] attribute {} expected {:?} got {:?}",
                    resource_type,
                    title,
                    key,
                    expected,
                    actual
                ));
            }
        }
        Ok(())
    }

    /// Validate `that_requires` / `that_comes_before` / `that_notifies` /
    /// `that_subscribes_to` references against the catalog's dependency edges.
    /// A relationship is satisfied whether it was declared on the subject or on
    /// the target (the inverse metaparameter), matching rspec-puppet.
    fn check_relationships(
        &self,
        catalog: &PuppetCatalog,
        resource_type: &str,
        title: &str,
        relationships: &[Relationship],
    ) -> Result<()> {
        if relationships.is_empty() {
            return Ok(());
        }
        let graph = build_dep_graph(catalog);
        let subject: ResourceRef = (normalize_rtype(resource_type), title.to_string());
        for rel in relationships {
            let Some(target) = parse_resource_ref(&rel.target) else {
                return Err(anyhow::anyhow!(
                    "relationship target {:?} is not a valid Type['title'] reference",
                    rel.target
                ));
            };
            let (satisfied, verb) = match rel.kind.as_str() {
                "require" => (
                    graph.before.contains(&(target.clone(), subject.clone())),
                    "require",
                ),
                "before" => (
                    graph.before.contains(&(subject.clone(), target.clone())),
                    "come before",
                ),
                "notify" => (
                    graph.notify.contains(&(subject.clone(), target.clone())),
                    "notify",
                ),
                "subscribe" => (
                    graph.notify.contains(&(target.clone(), subject.clone())),
                    "subscribe to",
                ),
                other => {
                    return Err(anyhow::anyhow!("unknown relationship kind {:?}", other));
                }
            };
            if !satisfied {
                return Err(anyhow::anyhow!(
                    "expected {}[{}] to {} {}[{}], but no such relationship is in the catalog",
                    subject.0,
                    subject.1,
                    verb,
                    target.0,
                    target.1
                ));
            }
        }
        Ok(())
    }

    /// `compile.with_all_deps` — every relationship reference in the catalog
    /// must point at a resource that is actually declared.
    fn check_all_deps(&self, catalog: &PuppetCatalog) -> Result<()> {
        let mut missing = Vec::new();
        for resource in catalog.iter_resources() {
            for meta in RELATIONSHIP_METAPARAMS {
                let Some(value) = resource.attributes.get(meta) else {
                    continue;
                };
                let mut refs = Vec::new();
                collect_refs(value, &mut refs);
                for (rtype, rtitle) in refs {
                    if !catalog.contains(&rtype, &rtitle) {
                        missing.push(format!(
                            "{}[{}] -> {}[{}]",
                            resource.resource_type, resource.title, rtype, rtitle
                        ));
                    }
                }
            }
        }
        if missing.is_empty() {
            Ok(())
        } else {
            missing.sort();
            missing.dedup();
            Err(anyhow::anyhow!(
                "catalog has unresolved dependencies: {}",
                missing.join(", ")
            ))
        }
    }
}

fn regex_marker(value: &JsonValue) -> Option<&str> {
    let obj = value.as_object()?;
    if obj.len() != 1 {
        return None;
    }
    obj.get("__regex__").and_then(|v| v.as_str())
}

/// Synthesize the node-derived facts that real Puppet/PDK populate from the
/// node name (`let(:node) { 'foo.example.com' }`). Without these, a manifest
/// that defaults a parameter to `$fqdn` (or `$hostname`/`$domain`) renders
/// `undef` under the native evaluator. We set both the legacy top-scope facts
/// and the structured `networking` hash, plus `trusted['certname']`. Facts the
/// spec set explicitly always win — we only fill keys that are absent.
fn derive_node_facts(facts: &mut PuppetValue, node: Option<&str>) {
    let Some(node) = node.map(str::trim).filter(|n| !n.is_empty()) else {
        return;
    };
    let PuppetValue::Hash(map) = facts else {
        return;
    };

    let (hostname, domain) = match node.split_once('.') {
        Some((host, domain)) => (host.to_string(), domain.to_string()),
        None => (node.to_string(), String::new()),
    };

    let mut fill = |key: &str, value: PuppetValue| {
        map.entry(key.to_string()).or_insert(value);
    };
    fill("fqdn", PuppetValue::String(node.to_string()));
    fill("hostname", PuppetValue::String(hostname.clone()));
    fill("clientcert", PuppetValue::String(node.to_string()));
    if !domain.is_empty() {
        fill("domain", PuppetValue::String(domain.clone()));
    }

    // Structured `networking` fact mirrors the legacy values.
    let mut networking = IndexMap::new();
    networking.insert("fqdn".to_string(), PuppetValue::String(node.to_string()));
    networking.insert("hostname".to_string(), PuppetValue::String(hostname));
    if !domain.is_empty() {
        networking.insert("domain".to_string(), PuppetValue::String(domain));
    }
    fill("networking", PuppetValue::Hash(networking));

    // `trusted['certname']` defaults to the node's certname (the node name).
    let mut trusted = IndexMap::new();
    trusted.insert(
        "certname".to_string(),
        PuppetValue::String(node.to_string()),
    );
    fill("trusted", PuppetValue::Hash(trusted));
}

/// Does a `raise_error` message constraint match the actual error text?
/// A `{__regex__: ...}` marker matches as a regex, a bare string matches as a
/// substring (rspec checks the full message, but our error text carries extra
/// context wrapping, so substring is the lenient choice), and no constraint
/// matches any error.
/// Compile a regex that came from a spec (`%r{…}`). Ruby anchors `^`/`$` to
/// line boundaries by default, so enable multi-line mode — otherwise a
/// `/^Subsystem …$/` content matcher never matches a line in the middle of a
/// multi-line rendered file.
fn compile_spec_regex(src: &str) -> Result<Regex> {
    RegexBuilder::new(src)
        .multi_line(true)
        .build()
        .map_err(|err| anyhow::anyhow!("{err}"))
}

fn error_matches(pattern: Option<&JsonValue>, message: &str) -> Result<bool> {
    match pattern {
        None | Some(JsonValue::Null) => Ok(true),
        Some(value) => {
            if let Some(src) = regex_marker(value) {
                let regex = compile_spec_regex(src)
                    .with_context(|| format!("invalid raise_error regex /{src}/"))?;
                Ok(regex.is_match(message))
            } else if let Some(text) = value.as_str() {
                Ok(message.contains(text))
            } else {
                Ok(true)
            }
        }
    }
}

fn describe_pattern(pattern: Option<&JsonValue>) -> String {
    match pattern {
        Some(value) => {
            if let Some(src) = regex_marker(value) {
                format!("/{src}/")
            } else if let Some(text) = value.as_str() {
                format!("{text:?}")
            } else {
                "an error".to_string()
            }
        }
        None => "an error".to_string(),
    }
}

#[cfg(test)]
mod coverage_tests {
    use super::*;
    use std::fs;

    fn write_two_class_module() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let manifests = dir.path().join("manifests");
        fs::create_dir_all(&manifests).unwrap();
        fs::write(
            manifests.join("covered.pp"),
            "class covered {\n  notify { 'hello': }\n}\n",
        )
        .unwrap();
        fs::write(
            manifests.join("untouched.pp"),
            "class untouched {\n  notify { 'goodbye': }\n}\n",
        )
        .unwrap();
        dir
    }

    fn plan_for_class(name: &str) -> RegentPlan {
        RegentPlan {
            tests: vec![RegentTest {
                name: format!("{name} spec"),
                subject: name.to_string(),
                title: None,
                node: None,
                facts: None,
                params: None,
                expectations: vec![Expectation::Compile {
                    negate: false,
                    check_all_deps: false,
                }],
            }],
            load_errors: Vec::new(),
        }
    }

    fn write_failing_module() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let manifests = dir.path().join("manifests");
        fs::create_dir_all(&manifests).unwrap();
        fs::write(
            manifests.join("boom.pp"),
            "class boom {\n  fail('something went terribly wrong')\n}\n",
        )
        .unwrap();
        fs::write(
            manifests.join("ok.pp"),
            "class ok {\n  notify { 'fine': }\n}\n",
        )
        .unwrap();
        dir
    }

    fn raise_plan(subject: &str, message: Option<JsonValue>, negate: bool) -> RegentPlan {
        RegentPlan {
            tests: vec![RegentTest {
                name: format!("{subject} raises"),
                subject: subject.to_string(),
                title: None,
                node: None,
                facts: None,
                params: None,
                expectations: vec![Expectation::RaiseError { message, negate }],
            }],
            load_errors: Vec::new(),
        }
    }

    fn regex(src: &str) -> JsonValue {
        serde_json::json!({ "__regex__": src })
    }

    fn status(plan: RegentPlan, dir: &tempfile::TempDir) -> (TestStatus, Option<String>) {
        let runner = RegentSpecRunner::new(dir.path()).unwrap();
        let results = runner.run_plan(plan).unwrap();
        let case = &results.test_cases[0];
        (case.status.clone(), case.message.clone())
    }

    #[test]
    fn raise_error_matches_failing_compile() {
        let module = write_failing_module();
        // bare: any error
        assert_eq!(
            status(raise_plan("boom", None, false), &module).0,
            TestStatus::Passed
        );
        // regex against the failure message
        assert_eq!(
            status(
                raise_plan("boom", Some(regex("terribly wrong")), false),
                &module
            )
            .0,
            TestStatus::Passed
        );
        // substring match
        assert_eq!(
            status(
                raise_plan(
                    "boom",
                    Some(JsonValue::String("went terribly".into())),
                    false
                ),
                &module
            )
            .0,
            TestStatus::Passed
        );
    }

    #[test]
    fn raise_error_fails_on_wrong_message_or_no_error() {
        let module = write_failing_module();
        // message that does not appear -> fail
        let (st, msg) = status(
            raise_plan("boom", Some(regex("not present")), false),
            &module,
        );
        assert_eq!(st, TestStatus::Failed);
        assert!(msg.unwrap().contains("expected error to match"));
        // a clean class that compiles -> expecting a raise must fail
        let (st, msg) = status(raise_plan("ok", None, false), &module);
        assert_eq!(st, TestStatus::Failed);
        assert!(msg.unwrap().contains("expected compilation to raise"));
    }

    #[test]
    fn raise_error_negated() {
        let module = write_failing_module();
        // not_to raise_error on a clean class -> pass
        assert_eq!(
            status(raise_plan("ok", None, true), &module).0,
            TestStatus::Passed
        );
        // not_to raise_error on a failing class -> fail
        let (st, msg) = status(raise_plan("boom", None, true), &module);
        assert_eq!(st, TestStatus::Failed);
        assert!(msg.unwrap().contains("NOT to raise"));
    }

    #[test]
    fn coverage_marks_only_evaluated_manifest_files() {
        let module = write_two_class_module();
        let runner = RegentSpecRunner::new(module.path()).unwrap();
        let results = runner.run_plan(plan_for_class("covered")).unwrap();
        let report = results.coverage.expect("coverage report should be present");

        let covered = report
            .file_coverage
            .iter()
            .find(|(p, _)| p.ends_with("covered.pp"))
            .map(|(_, fc)| fc)
            .expect("covered.pp missing from report");
        let untouched = report
            .file_coverage
            .iter()
            .find(|(p, _)| p.ends_with("untouched.pp"))
            .map(|(_, fc)| fc)
            .expect("untouched.pp missing from report");

        assert!(covered.lines_covered > 0, "covered.pp should be touched");
        assert_eq!(untouched.lines_covered, 0, "untouched.pp must stay at 0");
        assert!(report.overall_coverage > 0.0);
        assert!(report.overall_coverage < 100.0);
    }

    #[test]
    fn node_name_derives_fqdn_hostname_domain() {
        // A class that defaults parameters to the node-derived facts, the way a
        // real module would (`String $api_server = $fqdn`). Without node-fact
        // derivation these render `undef`.
        let dir = tempfile::tempdir().unwrap();
        let manifests = dir.path().join("manifests");
        fs::create_dir_all(&manifests).unwrap();
        fs::write(
            manifests.join("servers.pp"),
            "class servers(\n  String $api_server = $fqdn,\n  String $host = $hostname,\n  String $dom = $domain,\n) {\n  notify { \"server-${api_server}-${host}-${dom}\": }\n}\n",
        )
        .unwrap();

        let plan = RegentPlan {
            tests: vec![RegentTest {
                name: "servers on node".to_string(),
                subject: "servers".to_string(),
                title: None,
                node: Some("web01.example.com".to_string()),
                facts: None,
                params: None,
                expectations: vec![Expectation::Contain {
                    resource_type: "notify".to_string(),
                    title: "server-web01.example.com-web01-example.com".to_string(),
                    attributes: HashMap::new(),
                    relationships: Vec::new(),
                    negate: false,
                }],
            }],
            load_errors: Vec::new(),
        };

        let (st, msg) = status(plan, &dir);
        assert_eq!(st, TestStatus::Passed, "{msg:?}");
    }
}
