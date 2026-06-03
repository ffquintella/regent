use anyhow::Result;
use serde_json::json;
use std::fs;
use std::path::Path;

use super::TestResults;

/// Generates test reports in various formats
pub struct TestReporter;

impl TestReporter {
    /// Generate a JSON report
    pub fn to_json(results: &TestResults) -> Result<String> {
        let json = json!({
            "test_type": results.test_type,
            "summary": {
                "total": results.total,
                "passed": results.passed,
                "failed": results.failed,
                "skipped": results.skipped,
                "pending": results.pending,
                "duration_ms": results.duration_ms,
            },
            "success": results.success(),
            "exit_code": results.exit_code,
            "test_cases": results.test_cases.iter().map(|tc| {
                json!({
                    "name": tc.name,
                    "status": tc.status.as_str(),
                    "duration_ms": tc.duration_ms,
                    "message": tc.message,
                })
            }).collect::<Vec<_>>(),
        });

        Ok(serde_json::to_string_pretty(&json)?)
    }

    /// Generate a human-readable report
    pub fn to_text(results: &TestResults) -> String {
        let mut report = String::new();
        let separator = "=".repeat(70);

        report.push_str(&format!("\n{}\n", separator));
        report.push_str(&format!("Test Report: {}\n", results.test_type));
        report.push_str(&format!("{}\n\n", separator));

        // Summary section
        report.push_str("Summary:\n");
        report.push_str(&format!("  Total Tests:  {}\n", results.total));
        report.push_str(&format!("  Passed:       {} ✓\n", results.passed));
        report.push_str(&format!("  Failed:       {} ✗\n", results.failed));
        report.push_str(&format!("  Skipped:      {}\n", results.skipped));
        report.push_str(&format!("  Pending:      {}\n", results.pending));
        report.push_str(&format!("  Duration:     {}ms\n", results.duration_ms));
        report.push_str(&format!(
            "  Status:       {}\n\n",
            if results.success() {
                "PASSED"
            } else {
                "FAILED"
            }
        ));

        // Test cases section
        if !results.test_cases.is_empty() {
            report.push_str("Test Cases:\n");
            for test_case in &results.test_cases {
                let status_symbol = match test_case.status {
                    super::TestStatus::Passed => "✓",
                    super::TestStatus::Failed => "✗",
                    super::TestStatus::Skipped => "⊘",
                    super::TestStatus::Pending => "⊗",
                };

                report.push_str(&format!(
                    "  {} {} ({}ms)\n",
                    status_symbol, test_case.name, test_case.duration_ms
                ));

                if let Some(msg) = &test_case.message {
                    report.push_str(&format!("      Message: {}\n", msg));
                }
            }
            report.push_str("\n");
        }

        // Output section
        if !results.stdout.is_empty() {
            report.push_str("Standard Output:\n");
            report.push_str(&format!("{}\n\n", results.stdout));
        }

        if !results.stderr.is_empty() {
            report.push_str("Standard Error:\n");
            report.push_str(&format!("{}\n\n", results.stderr));
        }

        report.push_str(&format!("{}\n", separator));
        report
    }

    /// Generate HTML report
    pub fn to_html(results: &TestResults) -> String {
        let mut html = String::new();

        html.push_str("<!DOCTYPE html>\n");
        html.push_str("<html>\n");
        html.push_str("<head>\n");
        html.push_str("  <meta charset='UTF-8'>\n");
        html.push_str(&format!(
            "  <title>Test Report - {}</title>\n",
            results.test_type
        ));
        html.push_str("  <style>\n");
        html.push_str("    body { font-family: Arial, sans-serif; margin: 20px; }\n");
        html.push_str("    .summary { background: #f5f5f5; padding: 15px; border-radius: 5px; margin-bottom: 20px; }\n");
        html.push_str("    .passed { color: #28a745; font-weight: bold; }\n");
        html.push_str("    .failed { color: #dc3545; font-weight: bold; }\n");
        html.push_str("    .skipped { color: #ffc107; }\n");
        html.push_str("    table { width: 100%; border-collapse: collapse; }\n");
        html.push_str("    th, td { border: 1px solid #ddd; padding: 12px; text-align: left; }\n");
        html.push_str("    th { background-color: #f2f2f2; }\n");
        html.push_str("  </style>\n");
        html.push_str("</head>\n");
        html.push_str("<body>\n");

        html.push_str(&format!("  <h1>Test Report: {}</h1>\n", results.test_type));
        html.push_str("  <div class='summary'>\n");
        html.push_str(&format!(
            "    <p>Status: <span class='{}'>{}</span></p>\n",
            if results.success() {
                "passed"
            } else {
                "failed"
            },
            if results.success() {
                "PASSED"
            } else {
                "FAILED"
            }
        ));
        html.push_str(&format!(
            "    <p>Total Tests: {} | Passed: {} | Failed: {} | Skipped: {} | Pending: {}</p>\n",
            results.total, results.passed, results.failed, results.skipped, results.pending
        ));
        html.push_str(&format!("    <p>Duration: {}ms</p>\n", results.duration_ms));
        html.push_str("  </div>\n");

        if !results.test_cases.is_empty() {
            html.push_str("  <h2>Test Cases</h2>\n");
            html.push_str("  <table>\n");
            html.push_str(
                "    <tr><th>Status</th><th>Name</th><th>Duration</th><th>Message</th></tr>\n",
            );

            for test_case in &results.test_cases {
                let status_class = match test_case.status {
                    super::TestStatus::Passed => "passed",
                    super::TestStatus::Failed => "failed",
                    super::TestStatus::Skipped => "skipped",
                    super::TestStatus::Pending => "skipped",
                };

                html.push_str(&format!("    <tr>\n"));
                html.push_str(&format!(
                    "      <td class='{}'>{}</td>\n",
                    status_class,
                    test_case.status.as_str()
                ));
                html.push_str(&format!("      <td>{}</td>\n", test_case.name));
                html.push_str(&format!("      <td>{}ms</td>\n", test_case.duration_ms));
                html.push_str(&format!(
                    "      <td>{}</td>\n",
                    test_case.message.as_ref().unwrap_or(&String::new())
                ));
                html.push_str("    </tr>\n");
            }

            html.push_str("  </table>\n");
        }

        html.push_str("</body>\n");
        html.push_str("</html>\n");

        html
    }

    /// Write report to file
    pub fn write_report(results: &TestResults, path: &Path, format: ReportFormat) -> Result<()> {
        let content = match format {
            ReportFormat::Json => Self::to_json(results)?,
            ReportFormat::Text => Self::to_text(results),
            ReportFormat::Html => Self::to_html(results),
        };

        fs::write(path, content)?;
        Ok(())
    }
}

/// Report format options
pub enum ReportFormat {
    Json,
    Text,
    Html,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tester::{TestCase, TestResults, TestStatus};

    fn create_sample_results() -> TestResults {
        let mut results = TestResults::new("unit");
        results.passed = 3;
        results.failed = 1;
        results.total = 4;
        results.duration_ms = 250;
        results.stdout = "Test output here\n".to_string();

        results.add_test_case(TestCase {
            name: "test_example_1".to_string(),
            status: TestStatus::Passed,
            duration_ms: 50,
            message: None,
        });

        results.add_test_case(TestCase {
            name: "test_example_2".to_string(),
            status: TestStatus::Failed,
            duration_ms: 100,
            message: Some("Assertion failed".to_string()),
        });

        results
    }

    #[test]
    fn test_to_json() {
        let results = create_sample_results();
        let json = TestReporter::to_json(&results).unwrap();

        assert!(json.contains("test_type"));
        assert!(json.contains("unit"));
        assert!(json.contains("passed"));
    }

    #[test]
    fn test_to_text() {
        let results = create_sample_results();
        let text = TestReporter::to_text(&results);

        assert!(text.contains("Test Report: unit"));
        assert!(text.contains("Total Tests:"));
        assert!(text.contains("Passed:"));
        assert!(text.contains("Failed:"));
    }

    #[test]
    fn test_to_html() {
        let results = create_sample_results();
        let html = TestReporter::to_html(&results);

        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Test Report: unit"));
        assert!(html.contains("FAILED"));
    }

    #[test]
    fn test_report_json_validity() {
        let results = create_sample_results();
        let json_str = TestReporter::to_json(&results).unwrap();

        // Parse back to verify valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["test_type"], "unit");
        // Check that summary exists and has total field
        assert!(parsed["summary"]["total"].is_number());
    }

    #[test]
    fn test_text_report_formatting() {
        let results = create_sample_results();
        let text = TestReporter::to_text(&results);

        // Check for expected sections
        assert!(text.contains("Summary:"));
        assert!(text.contains("Test Cases:"));
        assert!(text.contains("Standard Output:"));
    }

    #[test]
    fn test_html_report_table() {
        let results = create_sample_results();
        let html = TestReporter::to_html(&results);

        assert!(html.contains("<table>"));
        assert!(html.contains("</table>"));
        assert!(html.contains("<tr>"));
        assert!(html.contains("</tr>"));
    }

    #[test]
    fn test_report_with_no_test_cases() {
        let results = TestResults::new("unit");
        let text = TestReporter::to_text(&results);

        assert!(text.contains("Test Report: unit"));
        // Should not crash even with empty test cases
        assert!(text.len() > 0);
    }
}
