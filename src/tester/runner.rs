use anyhow::{Context, Result};
use std::process::Command;
use std::time::Instant;

use super::{TestConfig, TestResults, TestStatus, TestCase, CoverageReport};
use std::collections::HashMap;

/// Runs tests against Puppet modules
pub struct TestRunner<'a> {
    config: &'a TestConfig,
}

impl<'a> TestRunner<'a> {
    pub fn new(config: &'a TestConfig) -> Self {
        Self { config }
    }

    /// Execute RSpec-Puppet unit tests
    pub fn run_unit_tests(&self) -> Result<TestResults> {
        self.check_rspec_installed()
            .context("RSpec not found. Install with: gem install rspec-puppet")?;

        let start = Instant::now();
        let output = self.execute_rspec_command()?;
        let duration = start.elapsed().as_millis() as u64;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let mut results = TestResults::new("unit");
        results.stdout = stdout.clone();
        results.stderr = stderr.clone();
        results.exit_code = output.status.code().unwrap_or(-1);
        results.duration_ms = duration;

        // Parse output and extract test results
        self.parse_rspec_output(&stdout, &mut results)?;

        Ok(results)
    }

    /// Execute integration tests (placeholder)
    pub fn run_integration_tests(&self) -> Result<TestResults> {
        // For now, return empty results with a note
        let mut results = TestResults::new("integration");
        results.stdout = "Integration tests not yet implemented\n".to_string();
        Ok(results)
    }

    /// Execute acceptance tests (placeholder)
    pub fn run_acceptance_tests(&self) -> Result<TestResults> {
        // For now, return empty results with a note
        let mut results = TestResults::new("acceptance");
        results.stdout = "Acceptance tests not yet implemented\n".to_string();
        Ok(results)
    }

    /// Generate code coverage report
    pub fn generate_coverage(&self) -> Result<CoverageReport> {
        // Placeholder implementation
        Ok(CoverageReport {
            overall_coverage: 0.0,
            lines_covered: 0,
            lines_total: 0,
            branches_covered: 0,
            branches_total: 0,
            file_coverage: HashMap::new(),
        })
    }

    /// Check if RSpec is installed
    fn check_rspec_installed(&self) -> Result<bool> {
        let output = Command::new("gem")
            .args(&["list", "^rspec$"])
            .output()
            .context("Failed to check if RSpec is installed")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.contains("rspec"))
    }

    /// Execute the rspec command
    fn execute_rspec_command(&self) -> Result<std::process::Output> {
        let mut cmd = Command::new("rspec");

        // Set working directory to module path
        cmd.current_dir(&self.config.module_path);

        // Add RSpec options
        cmd.arg("--format")
            .arg("progress")
            .arg("--color");

        // Add coverage if requested
        if self.config.coverage {
            cmd.arg("--require")
                .arg("simplecov")
                .arg("--require")
                .arg("simplecov-console");
        }

        // Execute and capture output
        cmd.output()
            .context("Failed to execute RSpec command")
    }

    /// Parse RSpec output and extract test results
    fn parse_rspec_output(&self, output: &str, results: &mut TestResults) -> Result<()> {
        // Parse the output line by line
        for line in output.lines() {
            // Look for the summary line (e.g., "5 examples, 0 failures")
            if line.contains("example") || line.contains("spec") {
                self.parse_summary_line(line, results)?;
            }

            // Look for individual test results
            if line.contains("✓") || line.contains("✗") || line.contains("FAILED") {
                self.parse_test_line(line, results)?;
            }
        }

        // If no tests were parsed, try to detect from exit code
        if results.total == 0 {
            results.total = 1;
            if results.exit_code == 0 {
                results.passed = 1;
            } else {
                results.failed = 1;
            }
        }

        Ok(())
    }

    /// Parse summary line like "5 examples, 0 failures"
    fn parse_summary_line(&self, line: &str, results: &mut TestResults) -> Result<()> {
        // Extract number of examples
        if let Some(pos) = line.find("example") {
            let before = &line[..pos];
            if let Some(num_str) = before.split_whitespace().last() {
                if let Ok(total) = num_str.parse::<usize>() {
                    results.total = total;
                }
            }
        }

        // Extract number of failures
        if let Some(pos) = line.find("failure") {
            let before = &line[..pos];
            if let Some(num_str) = before.split_whitespace().last() {
                if let Ok(failed) = num_str.parse::<usize>() {
                    results.failed = failed;
                }
            }
        }

        // Calculate passed = total - failed
        if results.total > 0 && results.failed > 0 {
            results.passed = results.total - results.failed;
        } else if results.total > 0 {
            results.passed = results.total;
        }

        Ok(())
    }

    /// Parse individual test line
    fn parse_test_line(&self, line: &str, results: &mut TestResults) -> Result<()> {
        let trimmed = line.trim();
        
        // Extract test name and status
        let (name, status) = if trimmed.contains("✓") || trimmed.starts_with(".") {
            (trimmed.to_string(), TestStatus::Passed)
        } else if trimmed.contains("✗") || trimmed.starts_with("F") {
            (trimmed.to_string(), TestStatus::Failed)
        } else if trimmed.contains("pending") || trimmed.contains("skip") {
            (trimmed.to_string(), TestStatus::Skipped)
        } else {
            return Ok(());
        };

        // Create test case
        let test_case = TestCase {
            name,
            status,
            duration_ms: 0,
            message: None,
        };

        // Only add if it looks like a real test result
        if !trimmed.is_empty() && trimmed.len() > 2 {
            results.add_test_case(test_case);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runner_creation() {
        let config = TestConfig::new("/tmp/module", crate::tester::TestType::Unit);
        let runner = TestRunner::new(&config);
        assert_eq!(runner.config.test_type, crate::tester::TestType::Unit);
    }

    #[test]
    fn test_parse_summary_line_examples() {
        let config = TestConfig::new("/tmp/module", crate::tester::TestType::Unit);
        let runner = TestRunner::new(&config);
        let mut results = TestResults::new("unit");

        runner.parse_summary_line("5 examples, 0 failures", &mut results).unwrap();

        assert_eq!(results.total, 5);
        assert_eq!(results.passed, 5);
        assert_eq!(results.failed, 0);
    }

    #[test]
    fn test_parse_summary_line_with_failures() {
        let config = TestConfig::new("/tmp/module", crate::tester::TestType::Unit);
        let runner = TestRunner::new(&config);
        let mut results = TestResults::new("unit");

        runner.parse_summary_line("10 examples, 2 failures", &mut results).unwrap();

        assert_eq!(results.total, 10);
        assert_eq!(results.failed, 2);
        assert_eq!(results.passed, 8);
    }

    #[test]
    fn test_parse_rspec_output_simple() {
        let config = TestConfig::new("/tmp/module", crate::tester::TestType::Unit);
        let runner = TestRunner::new(&config);
        let mut results = TestResults::new("unit");

        let output = ".....\n\n5 examples, 0 failures\n";
        runner.parse_rspec_output(output, &mut results).unwrap();

        assert_eq!(results.total, 5);
        assert_eq!(results.passed, 5);
        assert_eq!(results.failed, 0);
    }

    #[test]
    fn test_parse_rspec_output_with_failures() {
        let config = TestConfig::new("/tmp/module", crate::tester::TestType::Unit);
        let runner = TestRunner::new(&config);
        let mut results = TestResults::new("unit");

        let output = "..F.F\n\n5 examples, 2 failures\n";
        runner.parse_rspec_output(output, &mut results).unwrap();

        assert_eq!(results.total, 5);
        assert_eq!(results.failed, 2);
        assert_eq!(results.passed, 3);
    }

    #[test]
    fn test_parse_empty_output() {
        let config = TestConfig::new("/tmp/module", crate::tester::TestType::Unit);
        let runner = TestRunner::new(&config);
        let mut results = TestResults::new("unit");

        // Simulate failed execution with no output
        results.exit_code = 1;
        runner.parse_rspec_output("", &mut results).unwrap();

        // Should have at least one test case (failure)
        assert!(results.total > 0);
    }

    #[test]
    fn test_integration_tests_placeholder() {
        let config = TestConfig::new("/tmp/module", crate::tester::TestType::Integration);
        let runner = TestRunner::new(&config);

        let results = runner.run_integration_tests().unwrap();
        assert_eq!(results.test_type, "integration");
        assert!(results.stdout.contains("not yet implemented"));
    }

    #[test]
    fn test_acceptance_tests_placeholder() {
        let config = TestConfig::new("/tmp/module", crate::tester::TestType::Acceptance);
        let runner = TestRunner::new(&config);

        let results = runner.run_acceptance_tests().unwrap();
        assert_eq!(results.test_type, "acceptance");
        assert!(results.stdout.contains("not yet implemented"));
    }

    #[test]
    fn test_coverage_report_generation() {
        let config = TestConfig::new("/tmp/module", crate::tester::TestType::Unit);
        let runner = TestRunner::new(&config);

        let coverage = runner.generate_coverage().unwrap();
        assert_eq!(coverage.overall_coverage, 0.0);
    }
}
