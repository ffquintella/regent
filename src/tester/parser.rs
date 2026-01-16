use anyhow::Result;
use std::collections::HashMap;

use super::{TestResults, TestCase, TestStatus};

/// Parses RSpec output and extracts structured data
pub struct RSpecParser;

impl RSpecParser {
    /// Parse RSpec JSON output
    pub fn parse_json(_json_content: &str) -> Result<TestResults> {
        // Parse JSON output from RSpec
        // This would typically use serde_json
        let results = TestResults::new("unit");
        Ok(results)
    }

    /// Parse RSpec progress output (dots and Fs)
    pub fn parse_progress(output: &str) -> TestResults {
        let mut results = TestResults::new("unit");

        for char in output.chars() {
            match char {
                '.' => {
                    results.passed += 1;
                    results.total += 1;
                }
                'F' => {
                    results.failed += 1;
                    results.total += 1;
                }
                'S' | '*' => {
                    results.skipped += 1;
                    results.total += 1;
                }
                'P' => {
                    results.pending += 1;
                    results.total += 1;
                }
                _ => {}
            }
        }

        results
    }

    /// Parse RSpec documentation format output
    pub fn parse_documentation(output: &str) -> TestResults {
        let mut results = TestResults::new("unit");

        for line in output.lines() {
            let trimmed = line.trim();

            // Skip empty lines
            if trimmed.is_empty() {
                continue;
            }

            // Parse test case lines
            if trimmed.contains("✓") {
                results.add_test_case(TestCase {
                    name: trimmed.replace("✓", "").trim().to_string(),
                    status: TestStatus::Passed,
                    duration_ms: 0,
                    message: None,
                });
            } else if trimmed.contains("✗") || trimmed.contains("FAILED") {
                results.add_test_case(TestCase {
                    name: trimmed.replace("✗", "").trim().to_string(),
                    status: TestStatus::Failed,
                    duration_ms: 0,
                    message: None,
                });
            } else if trimmed.contains("(SKIP)") || trimmed.contains("(PENDING)") {
                results.add_test_case(TestCase {
                    name: trimmed.replace("(SKIP)", "").replace("(PENDING)", "").trim().to_string(),
                    status: TestStatus::Skipped,
                    duration_ms: 0,
                    message: None,
                });
            }
        }

        results
    }

    /// Extract timing information from RSpec output
    pub fn extract_timing(output: &str) -> HashMap<String, u64> {
        let mut timings = HashMap::new();

        for line in output.lines() {
            // Look for lines like "Finished in 0.12345 seconds"
            if line.contains("second") {
                // Split and find numeric value
                for part in line.split_whitespace() {
                    if let Ok(duration) = part.parse::<f64>() {
                        let ms = (duration * 1000.0) as u64;
                        timings.insert("total".to_string(), ms);
                        return timings;
                    }
                }
            }
        }

        timings
    }

    /// Extract failure details from RSpec output
    pub fn extract_failures(output: &str) -> Vec<(String, String)> {
        let mut failures = Vec::new();
        let mut in_failure = false;
        let mut current_test = String::new();
        let mut current_message = String::new();

        for line in output.lines() {
            if line.contains("FAILED") || line.contains("1)") {
                in_failure = true;
                current_test = line.trim().to_string();
                current_message.clear();
            } else if in_failure && line.trim().is_empty() {
                if !current_test.is_empty() {
                    failures.push((current_test.clone(), current_message.clone()));
                    current_test.clear();
                    current_message.clear();
                    in_failure = false;
                }
            } else if in_failure {
                current_message.push('\n');
                current_message.push_str(line);
            }
        }

        if !current_test.is_empty() {
            failures.push((current_test, current_message));
        }

        failures
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_progress_all_passed() {
        let output = ".....";
        let results = RSpecParser::parse_progress(output);

        assert_eq!(results.total, 5);
        assert_eq!(results.passed, 5);
        assert_eq!(results.failed, 0);
    }

    #[test]
    fn test_parse_progress_with_failures() {
        let output = "..F.F";
        let results = RSpecParser::parse_progress(output);

        assert_eq!(results.total, 5);
        assert_eq!(results.passed, 3);
        assert_eq!(results.failed, 2);
    }

    #[test]
    fn test_parse_progress_with_skipped() {
        let output = "..S...";
        let results = RSpecParser::parse_progress(output);

        assert_eq!(results.total, 6);
        assert_eq!(results.passed, 5);
        assert_eq!(results.skipped, 1);
    }

    #[test]
    fn test_parse_progress_mixed_statuses() {
        let output = ".F*SP.";
        let results = RSpecParser::parse_progress(output);

        assert_eq!(results.total, 6);
        assert_eq!(results.passed, 2);
        assert_eq!(results.failed, 1);
        assert_eq!(results.skipped, 2);
        assert_eq!(results.pending, 1);
    }

    #[test]
    fn test_parse_documentation_format() {
        let output = "MyClass\n  ✓ does something\n  ✓ does something else\n";
        let results = RSpecParser::parse_documentation(output);

        assert_eq!(results.total, 2);
        assert_eq!(results.passed, 2);
    }

    #[test]
    fn test_parse_documentation_with_failures() {
        let output = "MyClass\n  ✓ does something\n  ✗ does something else\n";
        let results = RSpecParser::parse_documentation(output);

        assert_eq!(results.total, 2);
        assert_eq!(results.passed, 1);
        assert_eq!(results.failed, 1);
    }

    #[test]
    fn test_extract_timing() {
        let output = "Finished in 1.23456 seconds";
        let timings = RSpecParser::extract_timing(output);

        assert!(timings.contains_key("total"));
        let duration = timings.get("total").unwrap();
        assert!(*duration > 1000 && *duration < 1500);
    }

    #[test]
    fn test_extract_failures_single() {
        let output = "FAILED test_example\n  Error: Something went wrong\n";
        let failures = RSpecParser::extract_failures(output);

        assert_eq!(failures.len(), 1);
        assert!(failures[0].0.contains("FAILED"));
    }

    #[test]
    fn test_extract_failures_multiple() {
        let output = "FAILED test_1\n  Error 1\n\nFAILED test_2\n  Error 2\n";
        let failures = RSpecParser::extract_failures(output);

        assert_eq!(failures.len(), 2);
    }
}
