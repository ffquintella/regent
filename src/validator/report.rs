//! Report module - Generates validation reports in multiple formats

use crate::validator::ValidationReport;
use std::fmt::Write as FmtWrite;

/// Report generator for validation results
pub struct ReportGenerator;

impl ReportGenerator {
    /// Generate JSON report
    pub fn to_json(report: &ValidationReport) -> anyhow::Result<String> {
        let json = serde_json::to_string_pretty(report)?;
        Ok(json)
    }

    /// Generate text/human-readable report
    pub fn to_text(report: &ValidationReport) -> String {
        let mut output = String::new();

        writeln!(&mut output, "╔════════════════════════════════════════╗").ok();
        writeln!(&mut output, "║     VALIDATION REPORT                  ║").ok();
        writeln!(&mut output, "╚════════════════════════════════════════╝").ok();
        writeln!(&mut output).ok();

        // Header
        writeln!(&mut output, "Module: {}", report.module_path).ok();
        writeln!(&mut output, "Timestamp: {}", report.timestamp).ok();
        writeln!(&mut output).ok();

        // Overall Status
        let status_icon = match report.overall_status {
            crate::validator::ValidationStatus::Success => "✅",
            crate::validator::ValidationStatus::Warnings => "⚠️ ",
            crate::validator::ValidationStatus::Errors => "❌",
            crate::validator::ValidationStatus::Failed => "🚨",
        };

        writeln!(
            &mut output,
            "Status: {} {}",
            status_icon,
            match report.overall_status {
                crate::validator::ValidationStatus::Success => "SUCCESS",
                crate::validator::ValidationStatus::Warnings => "WARNINGS",
                crate::validator::ValidationStatus::Errors => "ERRORS",
                crate::validator::ValidationStatus::Failed => "FAILED",
            }
        )
        .ok();
        writeln!(&mut output).ok();

        // Summary
        writeln!(&mut output, "Summary:").ok();
        writeln!(&mut output, "  Total Issues: {}", report.total_issues).ok();
        writeln!(&mut output, "  Errors: {}", report.errors).ok();
        writeln!(&mut output, "  Warnings: {}", report.warnings).ok();
        writeln!(&mut output, "  Info: {}", report.info_count).ok();
        writeln!(&mut output, "  Auto-fixed: {}", report.auto_fixed).ok();
        writeln!(&mut output).ok();

        // Detailed Results
        if !report.results.is_empty() {
            writeln!(&mut output, "Validation Tool Results:").ok();
            writeln!(&mut output, "─────────────────────────").ok();

            for result in &report.results {
                writeln!(
                    &mut output,
                    "\n[{}] {}",
                    result.tool,
                    if result.success {
                        "✅ PASS"
                    } else {
                        "❌ FAIL"
                    }
                )
                .ok();
                writeln!(&mut output, "  Issues Found: {}", result.issues.len()).ok();
                writeln!(
                    &mut output,
                    "  Execution Time: {}ms",
                    result.execution_time_ms
                )
                .ok();

                if !result.issues.is_empty() {
                    writeln!(&mut output, "  Issues:").ok();
                    for issue in &result.issues {
                        let level_icon = match issue.level {
                            crate::validator::LintLevel::Error => "❌",
                            crate::validator::LintLevel::Warning => "⚠️ ",
                            crate::validator::LintLevel::Info => "ℹ️ ",
                        };

                        writeln!(
                            &mut output,
                            "    {} [{}] {}: {}",
                            level_icon,
                            issue.code,
                            issue.file.display(),
                            issue.message
                        )
                        .ok();

                        if let Some(line) = issue.line {
                            write!(&mut output, " (line {}", line).ok();
                            if let Some(col) = issue.column {
                                write!(&mut output, ", col {}", col).ok();
                            }
                            writeln!(&mut output, ")").ok();
                        }
                    }
                }
            }
        }

        writeln!(&mut output).ok();
        writeln!(&mut output, "═════════════════════════════════════════").ok();

        output
    }

    /// Generate HTML report
    pub fn to_html(report: &ValidationReport) -> String {
        let status_class = match report.overall_status {
            crate::validator::ValidationStatus::Success => "success",
            crate::validator::ValidationStatus::Warnings => "warning",
            crate::validator::ValidationStatus::Errors => "error",
            crate::validator::ValidationStatus::Failed => "failed",
        };

        let status_text = match report.overall_status {
            crate::validator::ValidationStatus::Success => "SUCCESS",
            crate::validator::ValidationStatus::Warnings => "WARNINGS",
            crate::validator::ValidationStatus::Errors => "ERRORS",
            crate::validator::ValidationStatus::Failed => "FAILED",
        };

        let mut html = String::new();

        html.push_str("<!DOCTYPE html>\n");
        html.push_str("<html>\n");
        html.push_str("<head>\n");
        html.push_str("<meta charset='UTF-8'>\n");
        html.push_str("<title>Validation Report</title>\n");
        html.push_str("<style>\n");
        html.push_str(
            "body { font-family: Arial, sans-serif; margin: 20px; background: #f5f5f5; }\n",
        );
        html.push_str(".container { max-width: 1000px; margin: 0 auto; background: white; padding: 20px; border-radius: 8px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }\n");
        html.push_str(
            "h1 { color: #333; border-bottom: 3px solid #007bff; padding-bottom: 10px; }\n",
        );
        html.push_str(
            ".status { padding: 10px; border-radius: 4px; margin: 10px 0; font-weight: bold; }\n",
        );
        html.push_str(".status.success { background: #d4edda; color: #155724; }\n");
        html.push_str(".status.warning { background: #fff3cd; color: #856404; }\n");
        html.push_str(".status.error { background: #f8d7da; color: #721c24; }\n");
        html.push_str(".status.failed { background: #f8d7da; color: #721c24; }\n");
        html.push_str(".summary { background: #f8f9fa; padding: 15px; border-radius: 4px; margin: 15px 0; }\n");
        html.push_str(".summary-item { display: inline-block; margin-right: 30px; }\n");
        html.push_str(".summary-item strong { color: #007bff; }\n");
        html.push_str(".result { border: 1px solid #ddd; border-radius: 4px; padding: 15px; margin: 15px 0; }\n");
        html.push_str(
            ".result-header { font-weight: bold; font-size: 16px; margin-bottom: 10px; }\n",
        );
        html.push_str(".issue { background: #f9f9f9; padding: 10px; margin: 8px 0; border-left: 4px solid #007bff; }\n");
        html.push_str(".issue.error { border-left-color: #dc3545; }\n");
        html.push_str(".issue.warning { border-left-color: #ffc107; }\n");
        html.push_str(".issue.info { border-left-color: #17a2b8; }\n");
        html.push_str(".issue-code { font-weight: bold; color: #007bff; }\n");
        html.push_str(
            ".footer { text-align: center; color: #666; margin-top: 20px; font-size: 12px; }\n",
        );
        html.push_str("</style>\n");
        html.push_str("</head>\n");
        html.push_str("<body>\n");
        html.push_str("<div class='container'>\n");

        html.push_str("<h1>Validation Report</h1>\n");
        html.push_str(&format!(
            "<div class='status {}'>{}</div>\n",
            status_class, status_text
        ));

        html.push_str("<div class='summary'>\n");
        html.push_str(&format!(
            "<div class='summary-item'><strong>Module:</strong> {}</div>\n",
            report.module_path
        ));
        html.push_str(&format!(
            "<div class='summary-item'><strong>Timestamp:</strong> {}</div>\n",
            report.timestamp
        ));
        html.push_str(&format!(
            "<div class='summary-item'><strong>Total Issues:</strong> {}</div>\n",
            report.total_issues
        ));
        html.push_str(&format!(
            "<div class='summary-item'><strong>Errors:</strong> {}</div>\n",
            report.errors
        ));
        html.push_str(&format!(
            "<div class='summary-item'><strong>Warnings:</strong> {}</div>\n",
            report.warnings
        ));
        html.push_str("</div>\n");

        for result in &report.results {
            let result_status = if result.success {
                "✅ PASS"
            } else {
                "❌ FAIL"
            };
            html.push_str("<div class='result'>\n");
            html.push_str(&format!(
                "<div class='result-header'>{} - {}</div>\n",
                result.tool, result_status
            ));
            html.push_str(&format!(
                "<p>Issues: {} | Time: {}ms</p>\n",
                result.issues.len(),
                result.execution_time_ms
            ));

            if !result.issues.is_empty() {
                for issue in &result.issues {
                    let issue_class = match issue.level {
                        crate::validator::LintLevel::Error => "error",
                        crate::validator::LintLevel::Warning => "warning",
                        crate::validator::LintLevel::Info => "info",
                    };

                    html.push_str(&format!("<div class='issue {}'>\n", issue_class));
                    html.push_str(&format!(
                        "<span class='issue-code'>[{}]</span> {}\n",
                        issue.code, issue.message
                    ));
                    html.push_str(&format!("<br/><small>{}", issue.file.display()));
                    if let Some(line) = issue.line {
                        html.push_str(&format!(" (line {})", line));
                    }
                    html.push_str("</small>\n");
                    html.push_str("</div>\n");
                }
            }

            html.push_str("</div>\n");
        }

        html.push_str("<div class='footer'>\n");
        html.push_str("<p>Generated by Regent Validator</p>\n");
        html.push_str("</div>\n");

        html.push_str("</div>\n");
        html.push_str("</body>\n");
        html.push_str("</html>\n");

        html
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validator::ValidationStatus;

    fn create_test_report() -> ValidationReport {
        
        ValidationReport {
            module_path: "/test/module".to_string(),
            timestamp: "2026-01-16T00:00:00Z".to_string(),
            overall_status: ValidationStatus::Warnings,
            results: vec![],
            total_issues: 0,
            errors: 0,
            warnings: 0,
            info_count: 0,
            auto_fixed: 0,
        }
    }

    #[test]
    fn test_report_to_json() {
        let report = create_test_report();
        let json = ReportGenerator::to_json(&report);
        assert!(json.is_ok());
        let json_str = json.unwrap();
        assert!(json_str.contains("module_path"));
        assert!(json_str.contains("/test/module"));
    }

    #[test]
    fn test_report_to_text() {
        let report = create_test_report();
        let text = ReportGenerator::to_text(&report);
        assert!(text.contains("VALIDATION REPORT"));
        assert!(text.contains("/test/module"));
        assert!(text.contains("Status"));
    }

    #[test]
    fn test_report_to_html() {
        let report = create_test_report();
        let html = ReportGenerator::to_html(&report);
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Validation Report"));
        assert!(html.contains("/test/module"));
        assert!(html.contains("<style>"));
    }

    #[test]
    fn test_html_report_structure() {
        let report = create_test_report();
        let html = ReportGenerator::to_html(&report);
        assert!(html.contains("<html>"));
        assert!(html.contains("</html>"));
        assert!(html.contains("<head>"));
        assert!(html.contains("</head>"));
        assert!(html.contains("<body>"));
        assert!(html.contains("</body>"));
    }

    #[test]
    fn test_text_report_with_issues() {
        let mut report = create_test_report();
        report.total_issues = 1;
        report.errors = 1;

        let text = ReportGenerator::to_text(&report);
        assert!(text.contains("Total Issues: 1"));
        assert!(text.contains("Errors: 1"));
    }
}
