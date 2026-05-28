use anyhow::{Context, Result};
use regex::Regex;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use super::{CoverageReport, FileCoverage, TestCase, TestResults, TestStatus};
use crate::tester::puppet_eval::{
    EvaluationTrace, PuppetCatalog, PuppetEvaluator, PuppetValue,
};

#[derive(Debug, Deserialize)]
pub struct RegentPlan {
    pub tests: Vec<RegentTest>,
}

#[derive(Debug, Deserialize)]
pub struct RegentTest {
    pub name: String,
    pub subject: String,
    pub title: Option<String>,
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
    },
    #[serde(rename = "contain")]
    Contain {
        resource_type: String,
        title: String,
        #[serde(default)]
        attributes: HashMap<String, JsonValue>,
        #[serde(default)]
        negate: bool,
    },
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
        let facts = PuppetValue::from_json_map(test.facts.as_ref());
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
                Expectation::Compile { negate } => match (&catalog_result, *negate) {
                    (Ok(_), false) | (Err(_), true) => {}
                    (Err(err), false) => failures.push(format!("compile failed: {err}")),
                    (Ok(_), true) => failures.push("expected compile to fail, but it succeeded".to_string()),
                },
                Expectation::Contain {
                    resource_type,
                    title,
                    attributes,
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
                    let check = self.check_resource(catalog, resource_type, title, attributes);
                    match (check, *negate) {
                        (Ok(()), false) | (Err(_), true) => {}
                        (Err(err), false) => failures.push(err.to_string()),
                        (Ok(()), true) => failures.push(format!(
                            "expected catalog NOT to contain {}[{}] (with matching attrs), but it did",
                            resource_type, title
                        )),
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
                    message: Some(format!("compile failed: {err}")),
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
        } else {
            self.evaluator
                .evaluate_class_traced(subject, facts, params)
                .with_context(|| {
                    let known = self.evaluator.class_names();
                    if known.is_empty() {
                        format!(
                            "class {subject}: no classes were loaded \
                            (check that manifests/ exists and parses)"
                        )
                    } else {
                        let preview: Vec<String> = known.iter().take(20).cloned().collect();
                        format!(
                            "class {subject}: not found among {} loaded classes (e.g. {})",
                            known.len(),
                            preview.join(", ")
                        )
                    }
                })
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
            return Err(anyhow::anyhow!(
                "missing resource {}[{}]",
                resource_type,
                title
            ));
        };
        for (key, value) in attributes {
            let actual = resource
                .attributes
                .get(key)
                .cloned()
                .unwrap_or(PuppetValue::Undef);
            if let Some(pattern) = regex_marker(value) {
                let regex = Regex::new(pattern).map_err(|err| {
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
            if expected != actual {
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
}

fn regex_marker(value: &JsonValue) -> Option<&str> {
    let obj = value.as_object()?;
    if obj.len() != 1 {
        return None;
    }
    obj.get("__regex__").and_then(|v| v.as_str())
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
                facts: None,
                params: None,
                expectations: vec![Expectation::Compile { negate: false }],
            }],
        }
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
}
