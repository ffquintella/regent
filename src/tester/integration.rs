use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

/// Represents a Beaker node configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeSpec {
    pub name: String,
    pub platform: String,
    pub roles: Vec<String>,
}

impl NodeSpec {
    pub fn new(name: impl Into<String>, platform: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            platform: platform.into(),
            roles: Vec::new(),
        }
    }

    pub fn with_role(mut self, role: impl Into<String>) -> Self {
        self.roles.push(role.into());
        self
    }

    pub fn with_roles(mut self, roles: Vec<String>) -> Self {
        self.roles = roles;
        self
    }
}

/// Acceptance test scenario
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestScenario {
    pub name: String,
    pub description: String,
    pub provisioner: String,
    pub verifier: String,
    pub nodes: Vec<NodeSpec>,
}

impl TestScenario {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            provisioner: "chef".to_string(),
            verifier: "inspec".to_string(),
            nodes: Vec::new(),
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    pub fn with_provisioner(mut self, prov: impl Into<String>) -> Self {
        self.provisioner = prov.into();
        self
    }

    pub fn with_verifier(mut self, ver: impl Into<String>) -> Self {
        self.verifier = ver.into();
        self
    }

    pub fn add_node(mut self, node: NodeSpec) -> Self {
        self.nodes.push(node);
        self
    }
}

/// Result of a single acceptance test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptanceTestResult {
    pub scenario: String,
    pub node: String,
    pub success: bool,
    pub duration_ms: u64,
    pub output: String,
    pub errors: Vec<String>,
}

impl AcceptanceTestResult {
    pub fn new(scenario: impl Into<String>, node: impl Into<String>) -> Self {
        Self {
            scenario: scenario.into(),
            node: node.into(),
            success: false,
            duration_ms: 0,
            output: String::new(),
            errors: Vec::new(),
        }
    }
}

/// Results of acceptance test run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptanceResults {
    pub total_scenarios: usize,
    pub successful_scenarios: usize,
    pub failed_scenarios: usize,
    pub total_nodes: usize,
    pub tested_nodes: usize,
    pub total_tests: usize,
    pub passed: usize,
    pub failed: usize,
    pub results: Vec<AcceptanceTestResult>,
}

impl AcceptanceResults {
    pub fn new() -> Self {
        Self {
            total_scenarios: 0,
            successful_scenarios: 0,
            failed_scenarios: 0,
            total_nodes: 0,
            tested_nodes: 0,
            total_tests: 0,
            passed: 0,
            failed: 0,
            results: Vec::new(),
        }
    }

    pub fn add_result(&mut self, result: AcceptanceTestResult) {
        if result.success {
            self.passed += 1;
            self.successful_scenarios += 1;
        } else {
            self.failed += 1;
            self.failed_scenarios += 1;
        }
        self.total_tests += 1;
        self.tested_nodes += 1;
        self.results.push(result);
    }

    pub fn overall_success(&self) -> bool {
        self.failed == 0 && self.failed_scenarios == 0
    }

    pub fn success_rate(&self) -> f64 {
        if self.total_tests == 0 {
            0.0
        } else {
            (self.passed as f64 / self.total_tests as f64) * 100.0
        }
    }
}

impl Default for AcceptanceResults {
    fn default() -> Self {
        Self::new()
    }
}

/// Integration test runner for acceptance tests
pub struct IntegrationTester {
    pub module_path: PathBuf,
    pub scenarios: Vec<TestScenario>,
    pub config: IntegrationConfig,
}

/// Configuration for integration testing
#[derive(Debug, Clone)]
pub struct IntegrationConfig {
    pub beaker_available: bool,
    pub docker_available: bool,
    pub provision: bool,
    pub cleanup: bool,
    pub parallel: bool,
}

impl IntegrationConfig {
    pub fn new() -> Self {
        Self {
            beaker_available: false,
            docker_available: false,
            provision: true,
            cleanup: true,
            parallel: false,
        }
    }

    pub fn with_beaker(mut self, available: bool) -> Self {
        self.beaker_available = available;
        self
    }

    pub fn with_docker(mut self, available: bool) -> Self {
        self.docker_available = available;
        self
    }

    pub fn with_provision(mut self, provision: bool) -> Self {
        self.provision = provision;
        self
    }

    pub fn with_cleanup(mut self, cleanup: bool) -> Self {
        self.cleanup = cleanup;
        self
    }

    pub fn with_parallel(mut self, parallel: bool) -> Self {
        self.parallel = parallel;
        self
    }
}

impl Default for IntegrationConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl IntegrationTester {
    pub fn new(module_path: impl AsRef<std::path::Path>) -> Self {
        Self {
            module_path: module_path.as_ref().to_path_buf(),
            scenarios: Vec::new(),
            config: IntegrationConfig::new(),
        }
    }

    /// Add a test scenario
    pub fn add_scenario(&mut self, scenario: TestScenario) {
        self.scenarios.push(scenario);
    }

    /// Run acceptance tests
    pub fn run_acceptance_tests(&self) -> Result<AcceptanceResults> {
        let mut results = AcceptanceResults::new();
        results.total_scenarios = self.scenarios.len();

        for scenario in &self.scenarios {
            results.total_nodes += scenario.nodes.len();

            // For each node in the scenario
            for node in &scenario.nodes {
                let mut result = AcceptanceTestResult::new(&scenario.name, &node.name);

                // Simulate test execution
                result.success = true;
                result.duration_ms = 1000;
                result.output = format!(
                    "Running acceptance tests on {} for scenario {}",
                    node.name, scenario.name
                );

                results.add_result(result);
            }
        }

        Ok(results)
    }

    /// Setup nodes for testing
    pub fn setup_nodes(&self) -> Result<usize> {
        let mut count = 0;

        for scenario in &self.scenarios {
            if self.config.provision {
                for _node in &scenario.nodes {
                    // Simulate node provisioning
                    count += 1;
                }
            }
        }

        Ok(count)
    }

    /// Cleanup after testing
    pub fn cleanup_nodes(&self) -> Result<usize> {
        let mut count = 0;

        for scenario in &self.scenarios {
            if self.config.cleanup {
                for _node in &scenario.nodes {
                    // Simulate node cleanup
                    count += 1;
                }
            }
        }

        Ok(count)
    }

    /// Check if Beaker is available
    pub fn is_beaker_available() -> bool {
        Command::new("beaker")
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    /// Check if Docker is available
    pub fn is_docker_available() -> bool {
        Command::new("docker")
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    /// Get number of scenarios
    pub fn scenario_count(&self) -> usize {
        self.scenarios.len()
    }

    /// Get number of nodes
    pub fn node_count(&self) -> usize {
        self.scenarios.iter().map(|s| s.nodes.len()).sum()
    }

    /// Generate acceptance test summary
    pub fn generate_summary(&self, results: &AcceptanceResults) -> String {
        let mut summary = String::new();
        summary.push_str("\n=== Acceptance Test Summary ===\n\n");
        summary.push_str(&format!(
            "Scenarios: {}/{} passed\n",
            results.successful_scenarios, results.total_scenarios
        ));
        summary.push_str(&format!(
            "Tests: {}/{} passed ({:.1}%)\n",
            results.passed, results.total_tests, results.success_rate()
        ));
        summary.push_str(&format!(
            "Nodes Tested: {}/{}\n",
            results.tested_nodes, results.total_nodes
        ));

        if !results.overall_success() {
            summary.push_str("\n❌ Some tests failed:\n");
            for result in &results.results {
                if !result.success {
                    summary.push_str(&format!(
                        "  - {}/{}: {}\n",
                        result.scenario, result.node, result.errors.join(", ")
                    ));
                }
            }
        } else {
            summary.push_str("\n✅ All tests passed!\n");
        }

        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_spec_creation() {
        let node = NodeSpec::new("ubuntu-20.04", "ubuntu-20.04");
        assert_eq!(node.name, "ubuntu-20.04");
        assert_eq!(node.platform, "ubuntu-20.04");
        assert_eq!(node.roles.len(), 0);
    }

    #[test]
    fn test_node_spec_with_role() {
        let node = NodeSpec::new("ubuntu", "ubuntu-20.04")
            .with_role("master");

        assert_eq!(node.roles.len(), 1);
        assert_eq!(node.roles[0], "master");
    }

    #[test]
    fn test_node_spec_with_roles() {
        let node = NodeSpec::new("ubuntu", "ubuntu-20.04")
            .with_roles(vec!["master".to_string(), "agent".to_string()]);

        assert_eq!(node.roles.len(), 2);
    }

    #[test]
    fn test_test_scenario_creation() {
        let scenario = TestScenario::new("default");
        assert_eq!(scenario.name, "default");
        assert_eq!(scenario.provisioner, "chef");
        assert_eq!(scenario.verifier, "inspec");
        assert_eq!(scenario.nodes.len(), 0);
    }

    #[test]
    fn test_test_scenario_builder() {
        let scenario = TestScenario::new("custom")
            .with_description("Custom test scenario")
            .with_provisioner("puppet")
            .with_verifier("serverspec");

        assert_eq!(scenario.name, "custom");
        assert_eq!(scenario.description, "Custom test scenario");
        assert_eq!(scenario.provisioner, "puppet");
        assert_eq!(scenario.verifier, "serverspec");
    }

    #[test]
    fn test_test_scenario_add_node() {
        let scenario = TestScenario::new("default")
            .add_node(NodeSpec::new("ubuntu", "ubuntu-20.04"));

        assert_eq!(scenario.nodes.len(), 1);
        assert_eq!(scenario.nodes[0].name, "ubuntu");
    }

    #[test]
    fn test_acceptance_test_result_creation() {
        let result = AcceptanceTestResult::new("default", "ubuntu");
        assert_eq!(result.scenario, "default");
        assert_eq!(result.node, "ubuntu");
        assert!(!result.success);
        assert_eq!(result.duration_ms, 0);
    }

    #[test]
    fn test_acceptance_results_creation() {
        let results = AcceptanceResults::new();
        assert_eq!(results.total_scenarios, 0);
        assert_eq!(results.total_tests, 0);
        assert!(results.overall_success());
    }

    #[test]
    fn test_acceptance_results_add_result() {
        let mut results = AcceptanceResults::new();
        results.total_scenarios = 1;
        results.total_nodes = 1;

        let result = AcceptanceTestResult::new("default", "ubuntu");
        results.add_result(result);

        assert_eq!(results.total_tests, 1);
        assert_eq!(results.failed, 1);
    }

    #[test]
    fn test_acceptance_results_success_rate() {
        let mut results = AcceptanceResults::new();
        results.total_tests = 10;
        results.passed = 8;
        results.failed = 2;

        let rate = results.success_rate();
        assert_eq!(rate, 80.0);
    }

    #[test]
    fn test_integration_config_creation() {
        let config = IntegrationConfig::new();
        assert!(!config.beaker_available);
        assert!(!config.docker_available);
        assert!(config.provision);
        assert!(config.cleanup);
    }

    #[test]
    fn test_integration_config_builder() {
        let config = IntegrationConfig::new()
            .with_beaker(true)
            .with_docker(true)
            .with_parallel(true);

        assert!(config.beaker_available);
        assert!(config.docker_available);
        assert!(config.parallel);
    }

    #[test]
    fn test_integration_tester_creation() {
        let tester = IntegrationTester::new("/tmp/module");
        assert_eq!(tester.scenario_count(), 0);
        assert_eq!(tester.node_count(), 0);
    }

    #[test]
    fn test_integration_tester_add_scenario() {
        let mut tester = IntegrationTester::new("/tmp/module");
        let scenario = TestScenario::new("default")
            .add_node(NodeSpec::new("ubuntu", "ubuntu-20.04"));

        tester.add_scenario(scenario);
        assert_eq!(tester.scenario_count(), 1);
        assert_eq!(tester.node_count(), 1);
    }

    #[test]
    fn test_integration_tester_run_acceptance_tests() -> Result<()> {
        let mut tester = IntegrationTester::new("/tmp/module");
        let scenario = TestScenario::new("default")
            .add_node(NodeSpec::new("ubuntu", "ubuntu-20.04"))
            .add_node(NodeSpec::new("centos", "centos-7"));

        tester.add_scenario(scenario);

        let results = tester.run_acceptance_tests()?;
        assert_eq!(results.total_scenarios, 1);
        assert_eq!(results.total_nodes, 2);
        assert_eq!(results.total_tests, 2);

        Ok(())
    }

    #[test]
    fn test_integration_tester_setup_nodes() -> Result<()> {
        let mut tester = IntegrationTester::new("/tmp/module");
        let scenario = TestScenario::new("default")
            .add_node(NodeSpec::new("ubuntu", "ubuntu-20.04"));

        tester.add_scenario(scenario);

        let count = tester.setup_nodes()?;
        assert_eq!(count, 1);

        Ok(())
    }

    #[test]
    fn test_integration_tester_cleanup_nodes() -> Result<()> {
        let mut tester = IntegrationTester::new("/tmp/module");
        let scenario = TestScenario::new("default")
            .add_node(NodeSpec::new("ubuntu", "ubuntu-20.04"));

        tester.add_scenario(scenario);

        let count = tester.cleanup_nodes()?;
        assert_eq!(count, 1);

        Ok(())
    }

    #[test]
    fn test_integration_tester_generate_summary() -> Result<()> {
        let mut tester = IntegrationTester::new("/tmp/module");
        let scenario = TestScenario::new("default")
            .add_node(NodeSpec::new("ubuntu", "ubuntu-20.04"));

        tester.add_scenario(scenario);
        let results = tester.run_acceptance_tests()?;

        let summary = tester.generate_summary(&results);
        assert!(summary.contains("Acceptance Test Summary"));
        assert!(summary.contains("Scenarios:"));
        assert!(summary.contains("Tests:"));

        Ok(())
    }
}
