use anyhow::{Context, Result};
use regex::Regex;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::path::Path;

use super::{TestCase, TestResults, TestStatus};
use crate::tester::puppet_eval::{PuppetCatalog, PuppetEvaluator, PuppetValue};

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
        for test in plan.tests {
            let case_result = self.run_test(&test)?;
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
        Ok(results)
    }

    fn run_test(&self, test: &RegentTest) -> Result<TestCase> {
        let facts = PuppetValue::from_json_map(test.facts.as_ref());
        let params = PuppetValue::from_json_map(test.params.as_ref());

        let catalog_result = self.evaluate_subject(test, &facts, &params);

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
    ) -> Result<PuppetCatalog> {
        let subject = test.subject.trim();
        let title = test.title.as_deref();
        if title.is_some() || self.evaluator.is_define(subject) {
            let title = title.unwrap_or(subject);
            self.evaluator
                .evaluate_define(subject, title, facts, params)
                .with_context(|| format!("define {subject}"))
        } else {
            self.evaluator
                .evaluate_class(subject, facts, params)
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
