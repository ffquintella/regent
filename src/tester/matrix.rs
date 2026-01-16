use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use super::{TestResults, TestConfig, TestType};

/// Represents a specific version combination for testing
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct VersionPair {
    pub puppet_version: String,
    pub ruby_version: String,
}

impl VersionPair {
    pub fn new(puppet: String, ruby: String) -> Self {
        Self {
            puppet_version: puppet,
            ruby_version: ruby,
        }
    }

    pub fn display(&self) -> String {
        format!("Puppet {} / Ruby {}", self.puppet_version, self.ruby_version)
    }
}

/// Matrix of versions to test against
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionMatrix {
    /// Puppet versions to test (e.g., ["6.0", "7.0", "8.0"])
    pub puppet_versions: Vec<String>,
    /// Ruby versions to test (e.g., ["2.7", "3.0", "3.1"])
    pub ruby_versions: Vec<String>,
}

impl VersionMatrix {
    pub fn new(puppet_versions: Vec<String>, ruby_versions: Vec<String>) -> Self {
        Self {
            puppet_versions,
            ruby_versions,
        }
    }

    /// Generate all version pairs in the matrix
    pub fn generate_pairs(&self) -> Vec<VersionPair> {
        let mut pairs = Vec::new();
        for puppet in &self.puppet_versions {
            for ruby in &self.ruby_versions {
                pairs.push(VersionPair::new(puppet.clone(), ruby.clone()));
            }
        }
        pairs
    }

    /// Count total combinations
    pub fn total_combinations(&self) -> usize {
        self.puppet_versions.len() * self.ruby_versions.len()
    }
}

/// Result from testing a single version pair
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionTestResult {
    pub version_pair: VersionPair,
    pub test_results: Option<TestResults>,
    pub success: bool,
    pub error: Option<String>,
    pub duration_ms: u64,
}

impl VersionTestResult {
    pub fn success(version_pair: VersionPair, test_results: TestResults, duration_ms: u64) -> Self {
        let success = test_results.success();
        Self {
            version_pair,
            test_results: Some(test_results),
            success,
            error: None,
            duration_ms,
        }
    }

    pub fn failure(version_pair: VersionPair, error: String, duration_ms: u64) -> Self {
        Self {
            version_pair,
            test_results: None,
            success: false,
            error: Some(error),
            duration_ms,
        }
    }
}

/// Complete results from a matrix test run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatrixResults {
    pub total_combinations: usize,
    pub successful: usize,
    pub failed: usize,
    pub errored: usize,
    pub total_duration_ms: u64,
    pub results: Vec<VersionTestResult>,
    pub compatibility_matrix: HashMap<String, Vec<(String, bool)>>,
}

impl MatrixResults {
    pub fn new() -> Self {
        Self {
            total_combinations: 0,
            successful: 0,
            failed: 0,
            errored: 0,
            total_duration_ms: 0,
            results: Vec::new(),
            compatibility_matrix: HashMap::new(),
        }
    }

    pub fn add_result(&mut self, result: VersionTestResult) {
        if result.success && result.test_results.is_some() {
            self.successful += 1;
        } else if result.error.is_some() {
            self.errored += 1;
        } else {
            self.failed += 1;
        }
        self.results.push(result);
    }

    pub fn success_rate(&self) -> f64 {
        if self.total_combinations == 0 {
            0.0
        } else {
            (self.successful as f64 / self.total_combinations as f64) * 100.0
        }
    }

    pub fn generate_compatibility_matrix(&mut self) {
        // Group results by Puppet version
        for result in &self.results {
            let puppet_ver = result.version_pair.puppet_version.clone();
            let ruby_ver = result.version_pair.ruby_version.clone();
            
            self.compatibility_matrix
                .entry(puppet_ver)
                .or_insert_with(Vec::new)
                .push((ruby_ver, result.success));
        }
    }
}

impl Default for MatrixResults {
    fn default() -> Self {
        Self::new()
    }
}

/// Runs tests across a matrix of Puppet and Ruby versions
pub struct TestMatrixRunner {
    matrix: VersionMatrix,
    module_path: PathBuf,
    test_type: TestType,
    parallel: bool,
    max_threads: usize,
}

impl TestMatrixRunner {
    pub fn new(matrix: VersionMatrix, module_path: PathBuf, test_type: TestType) -> Self {
        Self {
            matrix,
            module_path,
            test_type,
            parallel: false,
            max_threads: 1,
        }
    }

    pub fn parallel(mut self, enabled: bool) -> Self {
        self.parallel = enabled;
        self
    }

    pub fn with_threads(mut self, count: usize) -> Self {
        self.max_threads = count;
        self
    }

    /// Run tests sequentially across all version pairs
    pub fn run_matrix(&self) -> Result<MatrixResults> {
        let mut results = MatrixResults::new();
        let pairs = self.matrix.generate_pairs();
        results.total_combinations = pairs.len();

        for pair in pairs {
            // Simulate test execution for each version pair
            let result = self.test_version_pair(&pair)?;
            results.add_result(result);
        }

        results.generate_compatibility_matrix();
        Ok(results)
    }

    /// Run tests in parallel across version pairs
    pub fn run_parallel(&self) -> Result<MatrixResults> {
        let mut results = MatrixResults::new();
        let pairs = self.matrix.generate_pairs();
        results.total_combinations = pairs.len();

        // Use a thread pool or rayon for parallel execution
        let results_mutex = Arc::new(Mutex::new(Vec::new()));
        let thread_count = self.max_threads.min(pairs.len());

        // Simulate parallel execution (simplified)
        for pair in pairs {
            let result = self.test_version_pair(&pair)?;
            results.add_result(result);
        }

        results.generate_compatibility_matrix();
        Ok(results)
    }

    /// Test a single version pair
    fn test_version_pair(&self, pair: &VersionPair) -> Result<VersionTestResult> {
        // This is a placeholder that simulates testing a version pair
        // In a real implementation, this would:
        // 1. Set up the environment for the specific versions
        // 2. Run the tests
        // 3. Collect results
        
        let duration_ms = 500; // Simulated duration

        // For now, return success for testing
        let test_result = TestResults::new("unit");
        Ok(VersionTestResult::success(pair.clone(), test_result, duration_ms))
    }

    pub fn matrix(&self) -> &VersionMatrix {
        &self.matrix
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_pair_creation() {
        let pair = VersionPair::new("7.0".to_string(), "2.7".to_string());
        assert_eq!(pair.puppet_version, "7.0");
        assert_eq!(pair.ruby_version, "2.7");
    }

    #[test]
    fn test_version_pair_display() {
        let pair = VersionPair::new("7.0".to_string(), "2.7".to_string());
        assert_eq!(pair.display(), "Puppet 7.0 / Ruby 2.7");
    }

    #[test]
    fn test_version_matrix_creation() {
        let matrix = VersionMatrix::new(
            vec!["6.0".to_string(), "7.0".to_string()],
            vec!["2.7".to_string(), "3.0".to_string()],
        );

        assert_eq!(matrix.puppet_versions.len(), 2);
        assert_eq!(matrix.ruby_versions.len(), 2);
    }

    #[test]
    fn test_version_matrix_generate_pairs() {
        let matrix = VersionMatrix::new(
            vec!["6.0".to_string(), "7.0".to_string()],
            vec!["2.7".to_string(), "3.0".to_string()],
        );

        let pairs = matrix.generate_pairs();
        assert_eq!(pairs.len(), 4); // 2 * 2
    }

    #[test]
    fn test_version_matrix_total_combinations() {
        let matrix = VersionMatrix::new(
            vec!["6.0".to_string(), "7.0".to_string()],
            vec!["2.7".to_string(), "3.0".to_string(), "3.1".to_string()],
        );

        assert_eq!(matrix.total_combinations(), 6); // 2 * 3
    }

    #[test]
    fn test_version_test_result_success() {
        let pair = VersionPair::new("7.0".to_string(), "2.7".to_string());
        let test_results = TestResults::new("unit");
        let result = VersionTestResult::success(pair, test_results, 500);

        assert!(result.success);
        assert_eq!(result.duration_ms, 500);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_version_test_result_failure() {
        let pair = VersionPair::new("7.0".to_string(), "2.7".to_string());
        let result = VersionTestResult::failure(pair, "Test failed".to_string(), 500);

        assert!(!result.success);
        assert_eq!(result.error.as_ref().unwrap(), "Test failed");
    }

    #[test]
    fn test_matrix_results_creation() {
        let results = MatrixResults::new();
        assert_eq!(results.total_combinations, 0);
        assert_eq!(results.successful, 0);
        assert_eq!(results.failed, 0);
    }

    #[test]
    fn test_matrix_results_add_result() {
        let mut results = MatrixResults::new();
        results.total_combinations = 1;

        let pair = VersionPair::new("7.0".to_string(), "2.7".to_string());
        let test_results = TestResults::new("unit");
        let result = VersionTestResult::success(pair, test_results, 500);

        results.add_result(result);

        assert_eq!(results.successful, 1);
        assert_eq!(results.failed, 0);
    }

    #[test]
    fn test_matrix_results_success_rate() {
        let mut results = MatrixResults::new();
        results.total_combinations = 2;

        let pair1 = VersionPair::new("7.0".to_string(), "2.7".to_string());
        let test_results1 = TestResults::new("unit");
        results.add_result(VersionTestResult::success(pair1, test_results1, 500));

        let pair2 = VersionPair::new("7.0".to_string(), "3.0".to_string());
        let test_results2 = TestResults::new("unit");
        results.add_result(VersionTestResult::success(pair2, test_results2, 500));

        assert_eq!(results.success_rate(), 100.0);
    }

    #[test]
    fn test_matrix_results_mixed_success() {
        let mut results = MatrixResults::new();
        results.total_combinations = 2;

        let pair1 = VersionPair::new("7.0".to_string(), "2.7".to_string());
        let test_results1 = TestResults::new("unit");
        results.add_result(VersionTestResult::success(pair1, test_results1, 500));

        let pair2 = VersionPair::new("7.0".to_string(), "3.0".to_string());
        results.add_result(VersionTestResult::failure(
            pair2,
            "Incompatible".to_string(),
            500,
        ));

        assert_eq!(results.success_rate(), 50.0);
    }

    #[test]
    fn test_matrix_runner_creation() {
        let matrix = VersionMatrix::new(
            vec!["7.0".to_string()],
            vec!["2.7".to_string()],
        );
        let runner = TestMatrixRunner::new(matrix, PathBuf::from("/tmp"), TestType::Unit);

        assert_eq!(runner.max_threads, 1);
        assert!(!runner.parallel);
    }

    #[test]
    fn test_matrix_runner_parallel_config() {
        let matrix = VersionMatrix::new(
            vec!["7.0".to_string()],
            vec!["2.7".to_string()],
        );
        let runner = TestMatrixRunner::new(matrix, PathBuf::from("/tmp"), TestType::Unit)
            .parallel(true)
            .with_threads(4);

        assert!(runner.parallel);
        assert_eq!(runner.max_threads, 4);
    }

    #[test]
    fn test_matrix_results_compatibility_matrix() {
        let mut results = MatrixResults::new();

        let pair1 = VersionPair::new("7.0".to_string(), "2.7".to_string());
        let test_results1 = TestResults::new("unit");
        results.add_result(VersionTestResult::success(pair1, test_results1, 500));

        let pair2 = VersionPair::new("7.0".to_string(), "3.0".to_string());
        let test_results2 = TestResults::new("unit");
        results.add_result(VersionTestResult::success(pair2, test_results2, 500));

        results.generate_compatibility_matrix();

        assert!(results.compatibility_matrix.contains_key("7.0"));
        let puppet_7_0 = results.compatibility_matrix.get("7.0").unwrap();
        assert_eq!(puppet_7_0.len(), 2);
    }
}
