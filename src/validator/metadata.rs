//! Metadata validator - Validates Puppet module metadata.json

use crate::validator::lint::{LintIssue, LintLevel, LintResult, LintTool};
use anyhow::Result;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Metadata JSON validator
pub struct MetadataValidator {
    module_path: PathBuf,
}

impl MetadataValidator {
    /// Create a new metadata validator
    pub fn new(module_path: impl AsRef<Path>) -> Result<Self> {
        let path = module_path.as_ref().to_path_buf();
        if !path.exists() {
            anyhow::bail!("Module path does not exist: {:?}", path);
        }

        Ok(Self { module_path: path })
    }

    /// Validate metadata.json file
    pub fn validate(&self) -> Result<LintResult> {
        let mut result = LintResult::new(LintTool::MetadataJsonLint);
        let start = std::time::Instant::now();

        let metadata_path = self.module_path.join("metadata.json");

        if !metadata_path.exists() {
            result.add_issue(
                LintIssue::new(
                    LintLevel::Error,
                    "E001".to_string(),
                    "metadata.json file not found".to_string(),
                    metadata_path,
                )
                .with_rule("missing_metadata".to_string()),
            );
            result.success = false;
        } else {
            // Validate JSON structure
            match self.validate_json_structure(&metadata_path) {
                Ok(issues) => result.add_issues(issues),
                Err(e) => {
                    result.add_issue(
                        LintIssue::new(
                            LintLevel::Error,
                            "E002".to_string(),
                            format!("Failed to parse metadata.json: {}", e),
                            metadata_path,
                        )
                        .with_rule("invalid_json".to_string()),
                    );
                }
            }
        }

        result.execution_time_ms = start.elapsed().as_millis();
        result.success = result.error_count() == 0;

        Ok(result)
    }

    fn validate_json_structure(&self, path: &Path) -> Result<Vec<LintIssue>> {
        let mut issues = Vec::new();

        let content = std::fs::read_to_string(path)?;
        let metadata: Value = serde_json::from_str(&content)?;

        // Validate required fields
        let required_fields = vec!["name", "version", "author", "summary"];
        for field in required_fields {
            if !metadata.get(field).is_some() {
                issues.push(
                    LintIssue::new(
                        LintLevel::Error,
                        "E003".to_string(),
                        format!("Missing required field: {}", field),
                        path.to_path_buf(),
                    )
                    .with_rule(format!("missing_{}", field)),
                );
            }
        }

        // Validate name format
        if let Some(name) = metadata.get("name").and_then(|v| v.as_str()) {
            if !name.contains('-') {
                issues.push(
                    LintIssue::new(
                        LintLevel::Warning,
                        "W001".to_string(),
                        "Module name should contain dash separator (e.g., namespace-modulename)"
                            .to_string(),
                        path.to_path_buf(),
                    )
                    .with_rule("invalid_name_format".to_string()),
                );
            }
        }

        // Validate version format
        if let Some(version) = metadata.get("version").and_then(|v| v.as_str()) {
            if !self.is_valid_version(version) {
                issues.push(
                    LintIssue::new(
                        LintLevel::Warning,
                        "W002".to_string(),
                        "Version should be in semantic versioning format (e.g., 1.0.0)".to_string(),
                        path.to_path_buf(),
                    )
                    .with_rule("invalid_version_format".to_string()),
                );
            }
        }

        Ok(issues)
    }

    fn is_valid_version(&self, version: &str) -> bool {
        let parts: Vec<&str> = version.split('.').collect();
        if parts.len() != 3 {
            return false;
        }

        parts.iter().all(|p| p.parse::<u32>().is_ok())
    }

    /// Get module path
    pub fn module_path(&self) -> &Path {
        &self.module_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata_validator_creation() {
        let result = MetadataValidator::new(".");
        assert!(result.is_ok());
    }

    #[test]
    fn test_metadata_validator_invalid_path() {
        let result = MetadataValidator::new("/nonexistent/metadata/path");
        assert!(result.is_err());
    }

    #[test]
    fn test_valid_version_format() {
        let validator = MetadataValidator::new(".").unwrap();
        assert!(validator.is_valid_version("1.0.0"));
        assert!(validator.is_valid_version("0.1.2"));
        assert!(validator.is_valid_version("10.20.30"));
        assert!(!validator.is_valid_version("1.0"));
        assert!(!validator.is_valid_version("1.0.0.0"));
        assert!(!validator.is_valid_version("a.b.c"));
    }

    #[test]
    fn test_metadata_validator_result() {
        let validator = MetadataValidator::new(".").unwrap();
        let result = validator.validate();
        assert!(result.is_ok());
        let validation = result.unwrap();
        assert_eq!(validation.tool, LintTool::MetadataJsonLint);
    }

    #[test]
    fn test_metadata_validator_getters() {
        let validator = MetadataValidator::new(".").unwrap();
        assert!(validator.module_path().exists());
    }
}
