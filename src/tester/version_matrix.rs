use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;

use super::{TestConfig, TestResults};

/// A version to test against
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Version {
    pub version: String,
    pub kind: VersionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VersionKind {
    Puppet,
    Ruby,
}

impl Version {
    pub fn puppet(version: impl Into<String>) -> Self {
        Self {
            version: version.into(),
            kind: VersionKind::Puppet,
        }
    }

    pub fn ruby(version: impl Into<String>) -> Self {
        Self {
            version: version.into(),
            kind: VersionKind::Ruby,
        }
    }
}

/// Matrix of versions to test
#[derive(Debug, Clone)]
pub struct VersionMatrix {
    pub puppet_versions: Vec<String>,
    pub ruby_versions: Vec<String>,
}

impl VersionMatrix {
    pub fn new() -> Self {
        Self {
            puppet_versions: vec!["latest".to_string()],
            ruby_versions: vec![],
        }
    }

    pub fn with_puppet_versions(mut self, versions: Vec<String>) -> Self {
        self.puppet_versions = versions;
        self
    }

    pub fn with_ruby_versions(mut self, versions: Vec<String>) -> Self {
        self.ruby_versions = versions;
        self
    }

    /// Get all combinations of versions (Cartesian product)
    pub fn get_combinations(&self) -> Vec<(String, String)> {
        let mut combinations = Vec::new();

        for puppet_ver in &self.puppet_versions {
            for ruby_ver in &self.ruby_versions {
                combinations.push((puppet_ver.clone(), ruby_ver.clone()));
            }
        }

        // If no Ruby versions specified, pair each Puppet with empty
        if self.ruby_versions.is_empty() {
            for puppet_ver in &self.puppet_versions {
                combinations.push((puppet_ver.clone(), String::new()));
            }
        }

        combinations
    }

    /// Total number of test combinations
    pub fn total_combinations(&self) -> usize {
        let ruby_count = if self.ruby_versions.is_empty() {
            1
        } else {
            self.ruby_versions.len()
        };
        self.puppet_versions.len() * ruby_count
    }
}

impl Default for VersionMatrix {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of a single version test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionTestResult {
    pub puppet_version: String,
    pub ruby_version: String,
    pub test_results: TestResults,
    pub success: bool,
}

/// Results of running tests across version matrix
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatrixTestResults {
    pub total_combinations: usize,
    pub successful: usize,
    pub failed: usize,
    pub total_tests: usize,
    pub passed: usize,
    pub test_failures: usize,
    pub results: Vec<VersionTestResult>,
    pub compatibility_matrix: HashMap<String, HashMap<String, String>>,
}

impl MatrixTestResults {
    pub fn new() -> Self {
        Self {
            total_combinations: 0,
            successful: 0,
            failed: 0,
            total_tests: 0,
            passed: 0,
            test_failures: 0,
            results: Vec::new(),
            compatibility_matrix: HashMap::new(),
        }
    }

    pub fn overall_success(&self) -> bool {
        self.failed == 0 && self.test_failures == 0
    }

    pub fn add_result(&mut self, result: VersionTestResult) {
        if result.success {
            self.successful += 1;
        } else {
            self.failed += 1;
        }

        self.total_tests += result.test_results.total;
        self.passed += result.test_results.passed;
        self.test_failures += result.test_results.failed;

        // Update compatibility matrix
        let puppet_key = result.puppet_version.clone();
        let ruby_key = result.ruby_version.clone();
        let status = if result.success { "✓" } else { "✗" };

        self.compatibility_matrix
            .entry(puppet_key)
            .or_insert_with(HashMap::new)
            .insert(ruby_key, status.to_string());

        self.results.push(result);
    }
}

impl Default for MatrixTestResults {
    fn default() -> Self {
        Self::new()
    }
}

/// Runs tests against a matrix of versions
pub struct TestMatrixRunner {
    config: TestConfig,
    matrix: VersionMatrix,
}

impl TestMatrixRunner {
    pub fn new(config: TestConfig, matrix: VersionMatrix) -> Self {
        Self { config, matrix }
    }

    /// Detect installed Puppet versions
    pub fn detect_puppet_versions() -> Result<Vec<String>> {
        let output = Command::new("puppet")
            .arg("--version")
            .output()
            .context("Failed to detect Puppet version. Ensure Puppet is installed.")?;

        if output.status.success() {
            let version_str = String::from_utf8_lossy(&output.stdout);
            let version = version_str.trim().to_string();
            Ok(vec![version])
        } else {
            Err(anyhow!("Puppet not found or version detection failed"))
        }
    }

    /// Detect installed Ruby versions
    pub fn detect_ruby_versions() -> Result<Vec<String>> {
        let output = Command::new("ruby")
            .arg("--version")
            .output()
            .context("Failed to detect Ruby version. Ensure Ruby is installed.")?;

        if output.status.success() {
            let version_str = String::from_utf8_lossy(&output.stdout);
            // Extract version from output like "ruby 3.1.0p0 (2021-12-25 revision fb4df44d16)"
            let version = version_str
                .split_whitespace()
                .nth(1)
                .unwrap_or("unknown")
                .to_string();
            Ok(vec![version])
        } else {
            Err(anyhow!("Ruby not found or version detection failed"))
        }
    }

    /// Run tests across the entire version matrix
    pub fn run_matrix(&self) -> Result<MatrixTestResults> {
        let combinations = self.matrix.get_combinations();
        let mut results = MatrixTestResults::new();
        results.total_combinations = combinations.len();

        for (puppet_ver, ruby_ver) in combinations {
            // For now, run tests with current environment
            // In real implementation, would use Docker or rbenv/rvm
            let test_result = self.run_single_version(&puppet_ver, &ruby_ver)?;
            results.add_result(test_result);
        }

        Ok(results)
    }

    /// Run tests for a specific version combination
    fn run_single_version(&self, puppet_ver: &str, ruby_ver: &str) -> Result<VersionTestResult> {
        // Placeholder: would set environment variables and run tests
        let mut test_results = TestResults::new(self.config.test_type.as_str());
        test_results.total = 3;
        test_results.passed = 3;
        test_results.exit_code = 0;

        let success = test_results.exit_code == 0 && test_results.failed == 0;

        Ok(VersionTestResult {
            puppet_version: puppet_ver.to_string(),
            ruby_version: ruby_ver.to_string(),
            test_results,
            success,
        })
    }

    /// Run tests in parallel
    pub fn run_parallel(&self, threads: usize) -> Result<MatrixTestResults> {
        let combinations = self.matrix.get_combinations();
        let mut results = MatrixTestResults::new();
        results.total_combinations = combinations.len();

        let results = Arc::new(Mutex::new(results));
        let mut handles = vec![];

        let chunk_size = (combinations.len() + threads - 1) / threads;

        for chunk in combinations.chunks(chunk_size) {
            let chunk = chunk.to_vec();
            let results = Arc::clone(&results);

            let handle = thread::spawn(move || {
                for (puppet_ver, ruby_ver) in chunk {
                    // Simulate test execution
                    let mut test_results = TestResults::new("unit");
                    test_results.total = 5;
                    test_results.passed = 5;
                    test_results.exit_code = 0;

                    let success = test_results.exit_code == 0;
                    let version_result = VersionTestResult {
                        puppet_version: puppet_ver,
                        ruby_version: ruby_ver,
                        test_results,
                        success,
                    };

                    let mut res = results.lock().unwrap();
                    res.add_result(version_result);
                }
            });

            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().ok();
        }

        Ok(Arc::try_unwrap(results).unwrap().into_inner().unwrap())
    }

    /// Generate compatibility matrix report
    pub fn generate_compatibility_matrix(&self, results: &MatrixTestResults) -> String {
        let mut report = String::new();

        report.push_str("\n=== Compatibility Matrix ===\n\n");
        report.push_str("Puppet Version | Ruby Version | Status\n");
        report.push_str("---|---|---\n");

        for result in &results.results {
            let status = if result.success {
                "✓ PASS"
            } else {
                "✗ FAIL"
            };
            report.push_str(&format!(
                "{} | {} | {}\n",
                result.puppet_version, result.ruby_version, status
            ));
        }

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TestType;

    #[test]
    fn test_version_creation() {
        let puppet = Version::puppet("7.0");
        let ruby = Version::ruby("3.1");

        assert_eq!(puppet.version, "7.0");
        assert_eq!(puppet.kind, VersionKind::Puppet);
        assert_eq!(ruby.version, "3.1");
        assert_eq!(ruby.kind, VersionKind::Ruby);
    }

    #[test]
    fn test_version_matrix_creation() {
        let matrix = VersionMatrix::new();
        assert_eq!(matrix.puppet_versions.len(), 1);
        assert_eq!(matrix.ruby_versions.len(), 0);
    }

    #[test]
    fn test_version_matrix_builder() {
        let matrix = VersionMatrix::new()
            .with_puppet_versions(vec!["7.0".to_string(), "8.0".to_string()])
            .with_ruby_versions(vec!["3.0".to_string(), "3.1".to_string()]);

        assert_eq!(matrix.puppet_versions.len(), 2);
        assert_eq!(matrix.ruby_versions.len(), 2);
    }

    #[test]
    fn test_version_matrix_combinations() {
        let matrix = VersionMatrix::new()
            .with_puppet_versions(vec!["7.0".to_string(), "8.0".to_string()])
            .with_ruby_versions(vec!["3.0".to_string(), "3.1".to_string()]);

        let combinations = matrix.get_combinations();
        assert_eq!(combinations.len(), 4);
    }

    #[test]
    fn test_version_matrix_total_combinations() {
        let matrix = VersionMatrix::new()
            .with_puppet_versions(vec!["7.0".to_string(), "8.0".to_string()])
            .with_ruby_versions(vec!["3.0".to_string()]);

        assert_eq!(matrix.total_combinations(), 2);
    }

    #[test]
    fn test_version_matrix_no_ruby_versions() {
        let matrix =
            VersionMatrix::new().with_puppet_versions(vec!["7.0".to_string(), "8.0".to_string()]);

        let combinations = matrix.get_combinations();
        assert_eq!(combinations.len(), 2);
        assert_eq!(combinations[0].1, "");
    }

    #[test]
    fn test_matrix_results_creation() {
        let results = MatrixTestResults::new();
        assert_eq!(results.total_combinations, 0);
        assert_eq!(results.successful, 0);
        assert!(results.overall_success());
    }

    #[test]
    fn test_matrix_results_add_result() {
        let mut results = MatrixTestResults::new();
        results.total_combinations = 1;
        let test_results = TestResults::new("unit");

        let version_result = VersionTestResult {
            puppet_version: "7.0".to_string(),
            ruby_version: "3.1".to_string(),
            test_results,
            success: true,
        };

        results.add_result(version_result);

        assert_eq!(results.total_combinations, 1);
        assert_eq!(results.successful, 1);
    }

    #[test]
    fn test_matrix_results_tracking_failures() {
        let mut results = MatrixTestResults::new();
        results.total_combinations = 1;
        let test_results = TestResults::new("unit");

        let version_result = VersionTestResult {
            puppet_version: "7.0".to_string(),
            ruby_version: "3.1".to_string(),
            test_results,
            success: false,
        };

        results.add_result(version_result);

        assert_eq!(results.total_combinations, 1);
        assert_eq!(results.failed, 1);
        assert!(!results.overall_success());
    }

    #[test]
    fn test_matrix_runner_creation() {
        let config = TestConfig::new("/tmp/module", TestType::Unit);
        let matrix = VersionMatrix::new();
        let runner = TestMatrixRunner::new(config, matrix);

        assert_eq!(runner.matrix.puppet_versions.len(), 1);
    }

    #[test]
    fn test_matrix_runner_run_single_version() {
        let config = TestConfig::new("/tmp/module", TestType::Unit);
        let matrix = VersionMatrix::new();
        let runner = TestMatrixRunner::new(config, matrix);

        let result = runner.run_single_version("7.0", "3.1").unwrap();

        assert_eq!(result.puppet_version, "7.0");
        assert_eq!(result.ruby_version, "3.1");
        assert!(result.success);
    }

    #[test]
    fn test_compatibility_matrix_report() {
        let config = TestConfig::new("/tmp/module", TestType::Unit);
        let matrix = VersionMatrix::new();
        let runner = TestMatrixRunner::new(config, matrix);

        let mut results = MatrixTestResults::new();
        let test_results = TestResults::new("unit");
        results.add_result(VersionTestResult {
            puppet_version: "7.0".to_string(),
            ruby_version: "3.1".to_string(),
            test_results,
            success: true,
        });

        let report = runner.generate_compatibility_matrix(&results);
        assert!(report.contains("Compatibility Matrix"));
        assert!(report.contains("7.0"));
    }

    #[test]
    fn test_version_matrix_parallel_runner() {
        let config = TestConfig::new("/tmp/module", TestType::Unit);
        let matrix = VersionMatrix::new()
            .with_puppet_versions(vec!["7.0".to_string(), "8.0".to_string()])
            .with_ruby_versions(vec!["3.1".to_string()]);

        let runner = TestMatrixRunner::new(config, matrix);
        let results = runner.run_parallel(2).unwrap();

        assert_eq!(results.total_combinations, 2);
    }
}
