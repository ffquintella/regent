//! CLI integration for validator commands

use crate::validator::{ModuleValidator, ValidatorConfig, ReportFormat, ReportGenerator};
use anyhow::Result;
use std::path::PathBuf;

/// Validator CLI command handler
pub struct ValidatorCli;

impl ValidatorCli {
    /// Handle validate command
    pub fn validate(
        module_path: &PathBuf,
        fail_on_warnings: bool,
        report_format: ReportFormat,
    ) -> Result<i32> {
        let config = ValidatorConfig {
            enable_puppet_lint: true,
            enable_puppet_syntax: true,
            enable_metadata_lint: true,
            enable_ruby_lint: false,
            enable_yaml_lint: false,
            fail_on_warnings,
            fail_on_errors: true,
        };

        let validator = ModuleValidator::new(module_path, config)?;
        let report = validator.validate_all()?;

        // Output report in requested format
        let output = match report_format {
            ReportFormat::Json => {
                ReportGenerator::to_json(&report)?
            },
            ReportFormat::Text => {
                ReportGenerator::to_text(&report)
            },
            ReportFormat::Html => {
                ReportGenerator::to_html(&report)
            },
        };

        println!("{}", output);

        // Determine exit code
        let exit_code = match (report.errors > 0, report.warnings > 0 && fail_on_warnings) {
            (true, _) => 1,
            (false, true) => 1,
            (false, false) => 0,
        };

        Ok(exit_code)
    }

    /// Handle validate specific tool
    pub fn validate_tool(
        module_path: &PathBuf,
        tool: &str,
    ) -> Result<i32> {
        let config = ValidatorConfig {
            enable_puppet_lint: tool == "puppet-lint" || tool == "all",
            enable_puppet_syntax: tool == "puppet-syntax" || tool == "all",
            enable_metadata_lint: tool == "metadata" || tool == "all",
            enable_ruby_lint: tool == "ruby" || tool == "all",
            enable_yaml_lint: tool == "yaml" || tool == "all",
            fail_on_warnings: false,
            fail_on_errors: true,
        };

        let validator = ModuleValidator::new(module_path, config)?;
        let report = validator.validate_all()?;

        println!("{}", ReportGenerator::to_text(&report));

        Ok(if report.errors > 0 { 1 } else { 0 })
    }

    /// List available validators
    pub fn list_validators() -> String {
        let mut output = String::new();
        output.push_str("Available Validators:\n");
        output.push_str("  - puppet-lint: Puppet code linting\n");
        output.push_str("  - puppet-syntax: Puppet syntax validation\n");
        output.push_str("  - metadata: Module metadata validation\n");
        output.push_str("  - ruby: Ruby code validation\n");
        output.push_str("  - yaml: YAML file validation\n");
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_validators() {
        let output = ValidatorCli::list_validators();
        assert!(output.contains("Available Validators"));
        assert!(output.contains("puppet-lint"));
        assert!(output.contains("puppet-syntax"));
        assert!(output.contains("metadata"));
    }

    #[test]
    fn test_cli_validate_current_dir() {
        let path = PathBuf::from(".");
        let result = ValidatorCli::validate(&path, false, ReportFormat::Text);
        assert!(result.is_ok());
    }

    #[test]
    fn test_cli_validate_tool_all() {
        let path = PathBuf::from(".");
        let result = ValidatorCli::validate_tool(&path, "all");
        assert!(result.is_ok());
    }

    #[test]
    fn test_cli_validate_tool_puppet_lint() {
        let path = PathBuf::from(".");
        let result = ValidatorCli::validate_tool(&path, "puppet-lint");
        assert!(result.is_ok());
    }

    #[test]
    fn test_cli_validate_tool_metadata() {
        let path = PathBuf::from(".");
        let result = ValidatorCli::validate_tool(&path, "metadata");
        assert!(result.is_ok());
    }
}
