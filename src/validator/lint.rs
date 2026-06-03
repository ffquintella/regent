//! Lint module - Core linting framework and data structures

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Lint level severity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
pub enum LintLevel {
    Info,
    Warning,
    Error,
}

impl std::fmt::Display for LintLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LintLevel::Info => write!(f, "INFO"),
            LintLevel::Warning => write!(f, "WARNING"),
            LintLevel::Error => write!(f, "ERROR"),
        }
    }
}

/// Lint tool type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LintTool {
    PuppetLint,
    PuppetSyntax,
    MetadataJsonLint,
    RubyLint,
    YamlLint,
}

impl std::fmt::Display for LintTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LintTool::PuppetLint => write!(f, "puppet-lint"),
            LintTool::PuppetSyntax => write!(f, "puppet-syntax"),
            LintTool::MetadataJsonLint => write!(f, "metadata-json-lint"),
            LintTool::RubyLint => write!(f, "ruby-lint"),
            LintTool::YamlLint => write!(f, "yaml-lint"),
        }
    }
}

/// Individual lint issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintIssue {
    pub level: LintLevel,
    pub code: String,
    pub message: String,
    pub file: PathBuf,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub rule: Option<String>,
}

impl LintIssue {
    pub fn new(level: LintLevel, code: String, message: String, file: PathBuf) -> Self {
        Self {
            level,
            code,
            message,
            file,
            line: None,
            column: None,
            rule: None,
        }
    }

    pub fn with_line(mut self, line: usize) -> Self {
        self.line = Some(line);
        self
    }

    pub fn with_column(mut self, column: usize) -> Self {
        self.column = Some(column);
        self
    }

    pub fn with_rule(mut self, rule: String) -> Self {
        self.rule = Some(rule);
        self
    }
}

/// Lint configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintConfig {
    pub enable_auto_fix: bool,
    pub strict_mode: bool,
    pub exclude_paths: Vec<String>,
    pub max_warnings: Option<usize>,
}

impl Default for LintConfig {
    fn default() -> Self {
        Self {
            enable_auto_fix: false,
            strict_mode: false,
            exclude_paths: vec![
                "spec".to_string(),
                "fixtures".to_string(),
                ".git".to_string(),
            ],
            max_warnings: None,
        }
    }
}

/// Lint result from a single tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintResult {
    pub tool: LintTool,
    pub success: bool,
    pub issues: Vec<LintIssue>,
    pub auto_fixed: usize,
    pub execution_time_ms: u128,
}

impl LintResult {
    pub fn new(tool: LintTool) -> Self {
        Self {
            tool,
            success: true,
            issues: Vec::new(),
            auto_fixed: 0,
            execution_time_ms: 0,
        }
    }

    pub fn add_issue(&mut self, issue: LintIssue) {
        self.issues.push(issue);
    }

    pub fn add_issues(&mut self, issues: Vec<LintIssue>) {
        self.issues.extend(issues);
    }

    pub fn error_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.level == LintLevel::Error)
            .count()
    }

    pub fn warning_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.level == LintLevel::Warning)
            .count()
    }

    pub fn info_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.level == LintLevel::Info)
            .count()
    }
}

/// Main lint manager for orchestrating multiple linters
pub struct LintManager {
    config: LintConfig,
}

impl LintManager {
    pub fn new(config: LintConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &LintConfig {
        &self.config
    }

    pub fn should_exclude(&self, path: &str) -> bool {
        self.config
            .exclude_paths
            .iter()
            .any(|exclude| path.contains(exclude))
    }
}

impl Default for LintManager {
    fn default() -> Self {
        Self::new(LintConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lint_level_ordering() {
        assert!(LintLevel::Info < LintLevel::Warning);
        assert!(LintLevel::Warning < LintLevel::Error);
    }

    #[test]
    fn test_lint_level_display() {
        assert_eq!(LintLevel::Info.to_string(), "INFO");
        assert_eq!(LintLevel::Warning.to_string(), "WARNING");
        assert_eq!(LintLevel::Error.to_string(), "ERROR");
    }

    #[test]
    fn test_lint_tool_display() {
        assert_eq!(LintTool::PuppetLint.to_string(), "puppet-lint");
        assert_eq!(LintTool::PuppetSyntax.to_string(), "puppet-syntax");
        assert_eq!(LintTool::MetadataJsonLint.to_string(), "metadata-json-lint");
        assert_eq!(LintTool::RubyLint.to_string(), "ruby-lint");
        assert_eq!(LintTool::YamlLint.to_string(), "yaml-lint");
    }

    #[test]
    fn test_lint_issue_builder() {
        let issue = LintIssue::new(
            LintLevel::Error,
            "E001".to_string(),
            "Test error".to_string(),
            PathBuf::from("test.pp"),
        )
        .with_line(10)
        .with_column(5)
        .with_rule("test_rule".to_string());

        assert_eq!(issue.level, LintLevel::Error);
        assert_eq!(issue.line, Some(10));
        assert_eq!(issue.column, Some(5));
        assert_eq!(issue.rule, Some("test_rule".to_string()));
    }

    #[test]
    fn test_lint_result_creation() {
        let result = LintResult::new(LintTool::PuppetLint);
        assert_eq!(result.tool, LintTool::PuppetLint);
        assert!(result.success);
        assert_eq!(result.issues.len(), 0);
    }

    #[test]
    fn test_lint_result_counts() {
        let mut result = LintResult::new(LintTool::PuppetLint);
        result.add_issue(LintIssue::new(
            LintLevel::Error,
            "E001".to_string(),
            "Error".to_string(),
            PathBuf::from("test.pp"),
        ));
        result.add_issue(LintIssue::new(
            LintLevel::Warning,
            "W001".to_string(),
            "Warning".to_string(),
            PathBuf::from("test.pp"),
        ));
        result.add_issue(LintIssue::new(
            LintLevel::Info,
            "I001".to_string(),
            "Info".to_string(),
            PathBuf::from("test.pp"),
        ));

        assert_eq!(result.error_count(), 1);
        assert_eq!(result.warning_count(), 1);
        assert_eq!(result.info_count(), 1);
    }

    #[test]
    fn test_lint_config_defaults() {
        let config = LintConfig::default();
        assert!(!config.enable_auto_fix);
        assert!(!config.strict_mode);
        assert_eq!(config.exclude_paths.len(), 3);
        assert_eq!(config.max_warnings, None);
    }

    #[test]
    fn test_lint_manager_exclusion() {
        let manager = LintManager::default();
        assert!(manager.should_exclude("spec/test.rb"));
        assert!(manager.should_exclude("fixtures/test.pp"));
        assert!(!manager.should_exclude("manifests/test.pp"));
    }

    #[test]
    fn test_lint_manager_creation() {
        let config = LintConfig {
            enable_auto_fix: true,
            strict_mode: true,
            exclude_paths: vec!["test".to_string()],
            max_warnings: Some(10),
        };
        let manager = LintManager::new(config);
        assert!(manager.config().enable_auto_fix);
        assert!(manager.config().strict_mode);
        assert_eq!(manager.config().max_warnings, Some(10));
    }
}
