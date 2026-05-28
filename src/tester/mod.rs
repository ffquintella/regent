use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::collections::HashMap;

pub mod runner;
pub mod parser;
pub mod reporter;
pub mod version_matrix;
pub mod fixtures;
pub mod integration;
pub mod artichoke_runner;
pub mod bundled_gems;
pub mod puppet_eval;
pub mod regent_spec;

pub use runner::TestRunner;
pub use artichoke_runner::ArtichokeTestRunner;
pub use parser::RSpecParser;
pub use reporter::TestReporter;
pub use version_matrix::{TestMatrixRunner, VersionMatrix, VersionTestResult, MatrixTestResults};
pub use fixtures::{FixtureManager, FixtureConfig, FixtureModule};
pub use integration::{IntegrationTester, NodeSpec, TestScenario, AcceptanceResults, IntegrationConfig};
pub use regent_spec::{RegentPlan, RegentSpecRunner};

/// Types of tests that can be executed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TestType {
    /// RSpec-Puppet unit tests
    Unit,
    /// Integration tests
    Integration,
    /// Acceptance tests (Beaker)
    Acceptance,
}

impl TestType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TestType::Unit => "unit",
            TestType::Integration => "integration",
            TestType::Acceptance => "acceptance",
        }
    }
}

/// Configuration for running tests
#[derive(Debug, Clone)]
pub struct TestConfig {
    /// Path to the module to test
    pub module_path: PathBuf,
    /// Type of test to run
    pub test_type: TestType,
    /// Optional RSpec pattern override
    pub pattern: Option<String>,
    /// Puppet version(s) to test against
    pub puppet_versions: Vec<String>,
    /// Ruby version(s) to test against
    pub ruby_versions: Vec<String>,
    /// Whether to run in parallel
    pub parallel: bool,
    /// Number of threads for parallel execution
    pub threads: Option<usize>,
    /// Generate coverage reports
    pub coverage: bool,
}

impl TestConfig {
    pub fn new(module_path: impl Into<PathBuf>, test_type: TestType) -> Self {
        Self {
            module_path: module_path.into(),
            test_type,
            pattern: None,
            puppet_versions: vec!["latest".to_string()],
            ruby_versions: vec![],
            parallel: false,
            threads: None,
            coverage: false,
        }
    }

    pub fn with_pattern(mut self, pattern: Option<String>) -> Self {
        self.pattern = pattern;
        self
    }

    pub fn with_puppet_versions(mut self, versions: Vec<String>) -> Self {
        self.puppet_versions = versions;
        self
    }

    pub fn with_ruby_versions(mut self, versions: Vec<String>) -> Self {
        self.ruby_versions = versions;
        self
    }

    pub fn parallel(mut self, enabled: bool) -> Self {
        self.parallel = enabled;
        self
    }

    pub fn with_threads(mut self, count: usize) -> Self {
        self.threads = Some(count);
        self
    }

    pub fn coverage(mut self, enabled: bool) -> Self {
        self.coverage = enabled;
        self
    }
}

/// Individual test case result
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestCase {
    pub name: String,
    pub status: TestStatus,
    pub duration_ms: u64,
    pub message: Option<String>,
}

/// Status of a test execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TestStatus {
    Passed,
    Failed,
    Skipped,
    Pending,
}

impl TestStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TestStatus::Passed => "passed",
            TestStatus::Failed => "failed",
            TestStatus::Skipped => "skipped",
            TestStatus::Pending => "pending",
        }
    }
}

/// Results from running tests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResults {
    pub test_type: String,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub pending: usize,
    pub duration_ms: u64,
    pub test_cases: Vec<TestCase>,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub coverage: Option<CoverageReport>,
}

impl TestResults {
    pub fn new(test_type: &str) -> Self {
        Self {
            test_type: test_type.to_string(),
            total: 0,
            passed: 0,
            failed: 0,
            skipped: 0,
            pending: 0,
            duration_ms: 0,
            test_cases: Vec::new(),
            stdout: String::new(),
            stderr: String::new(),
            exit_code: 0,
            coverage: None,
        }
    }

    pub fn success(&self) -> bool {
        self.failed == 0 && self.exit_code == 0
    }

    pub fn add_test_case(&mut self, test_case: TestCase) {
        match test_case.status {
            TestStatus::Passed => self.passed += 1,
            TestStatus::Failed => self.failed += 1,
            TestStatus::Skipped => self.skipped += 1,
            TestStatus::Pending => self.pending += 1,
        }
        self.total += 1;
        self.test_cases.push(test_case);
    }
}

/// Code coverage report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageReport {
    pub overall_coverage: f64,
    pub lines_covered: usize,
    pub lines_total: usize,
    pub branches_covered: usize,
    pub branches_total: usize,
    pub file_coverage: HashMap<String, FileCoverage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileCoverage {
    pub path: String,
    pub coverage: f64,
    pub lines_covered: usize,
    pub lines_total: usize,
}

/// Main test orchestrator
pub struct ModuleTester {
    config: TestConfig,
}

impl ModuleTester {
    pub fn new(config: TestConfig) -> Self {
        Self { config }
    }

    /// Run tests based on configuration
    pub fn run_tests(&self) -> Result<TestResults> {
        match self.config.test_type {
            TestType::Unit => self.run_unit_tests(),
            TestType::Integration => self.run_integration_tests(),
            TestType::Acceptance => self.run_acceptance_tests(),
        }
    }

    /// Run RSpec-Puppet unit tests
    pub fn run_unit_tests(&self) -> Result<TestResults> {
        let runner = ArtichokeTestRunner::new(&self.config);
        runner.run_unit_tests()
    }

    /// Run integration tests
    pub fn run_integration_tests(&self) -> Result<TestResults> {
        let runner = TestRunner::new(&self.config);
        runner.run_integration_tests()
    }

    /// Run acceptance tests
    pub fn run_acceptance_tests(&self) -> Result<TestResults> {
        let runner = TestRunner::new(&self.config);
        runner.run_acceptance_tests()
    }

    /// Generate coverage report
    pub fn generate_coverage(&self) -> Result<CoverageReport> {
        let runner = TestRunner::new(&self.config);
        runner.generate_coverage()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = TestConfig::new("/tmp/module", TestType::Unit)
            .with_puppet_versions(vec!["7.0".to_string(), "8.0".to_string()])
            .parallel(true)
            .with_threads(4)
            .coverage(true);

        assert_eq!(config.module_path, PathBuf::from("/tmp/module"));
        assert_eq!(config.test_type, TestType::Unit);
        assert_eq!(config.puppet_versions.len(), 2);
        assert!(config.parallel);
        assert_eq!(config.threads, Some(4));
        assert!(config.coverage);
    }

    #[test]
    fn test_test_type_as_str() {
        assert_eq!(TestType::Unit.as_str(), "unit");
        assert_eq!(TestType::Integration.as_str(), "integration");
        assert_eq!(TestType::Acceptance.as_str(), "acceptance");
    }

    #[test]
    fn test_test_status_as_str() {
        assert_eq!(TestStatus::Passed.as_str(), "passed");
        assert_eq!(TestStatus::Failed.as_str(), "failed");
        assert_eq!(TestStatus::Skipped.as_str(), "skipped");
        assert_eq!(TestStatus::Pending.as_str(), "pending");
    }

    #[test]
    fn test_test_results_creation() {
        let results = TestResults::new("unit");
        assert_eq!(results.test_type, "unit");
        assert_eq!(results.total, 0);
        assert_eq!(results.passed, 0);
        assert!(results.success());
    }

    #[test]
    fn test_results_add_test_case() {
        let mut results = TestResults::new("unit");

        let passed = TestCase {
            name: "test_example".to_string(),
            status: TestStatus::Passed,
            duration_ms: 100,
            message: None,
        };

        results.add_test_case(passed);

        assert_eq!(results.total, 1);
        assert_eq!(results.passed, 1);
        assert_eq!(results.test_cases.len(), 1);
    }

    #[test]
    fn test_results_failure_detection() {
        let mut results = TestResults::new("unit");

        let failed = TestCase {
            name: "test_failure".to_string(),
            status: TestStatus::Failed,
            duration_ms: 150,
            message: Some("Assertion failed".to_string()),
        };

        results.add_test_case(failed);

        assert_eq!(results.total, 1);
        assert_eq!(results.failed, 1);
        assert!(!results.success());
    }

    #[test]
    fn test_results_mixed_statuses() {
        let mut results = TestResults::new("unit");

        results.add_test_case(TestCase {
            name: "test_1".to_string(),
            status: TestStatus::Passed,
            duration_ms: 100,
            message: None,
        });

        results.add_test_case(TestCase {
            name: "test_2".to_string(),
            status: TestStatus::Skipped,
            duration_ms: 0,
            message: None,
        });

        results.add_test_case(TestCase {
            name: "test_3".to_string(),
            status: TestStatus::Pending,
            duration_ms: 50,
            message: None,
        });

        assert_eq!(results.total, 3);
        assert_eq!(results.passed, 1);
        assert_eq!(results.skipped, 1);
        assert_eq!(results.pending, 1);
    }

    #[test]
    fn test_coverage_report_creation() {
        let report = CoverageReport {
            overall_coverage: 85.5,
            lines_covered: 855,
            lines_total: 1000,
            branches_covered: 42,
            branches_total: 50,
            file_coverage: HashMap::new(),
        };

        assert_eq!(report.overall_coverage, 85.5);
        assert_eq!(report.lines_covered, 855);
    }

    #[test]
    fn test_module_tester_creation() {
        let config = TestConfig::new("/tmp/module", TestType::Unit);
        let tester = ModuleTester::new(config);
        assert_eq!(tester.config.test_type, TestType::Unit);
    }
}
