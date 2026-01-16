//! Validator Module - Comprehensive validation framework for Puppet modules
//!
//! This module provides a unified interface for validating Puppet modules,
//! supporting multiple validation tools including puppet-lint, puppet-syntax,
//! metadata-json-lint, and Ruby linting.

use std::path::{Path, PathBuf};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use chrono;

pub mod lint;
pub mod metadata;
pub mod puppet;
pub mod ruby;
pub mod yaml;

pub use lint::{LintConfig, LintIssue, LintLevel, LintManager, LintResult, LintTool};
pub use metadata::MetadataValidator;
pub use puppet::PuppetValidator;
pub use ruby::RubyValidator;
pub use yaml::YamlValidator;

/// Main validator orchestrator
pub struct ModuleValidator {
    module_path: PathBuf,
    config: ValidatorConfig,
}

/// Validator configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorConfig {
    pub enable_puppet_lint: bool,
    pub enable_puppet_syntax: bool,
    pub enable_metadata_lint: bool,
    pub enable_ruby_lint: bool,
    pub enable_yaml_lint: bool,
    pub fail_on_warnings: bool,
    pub fail_on_errors: bool,
}

impl Default for ValidatorConfig {
    fn default() -> Self {
        Self {
            enable_puppet_lint: true,
            enable_puppet_syntax: true,
            enable_metadata_lint: true,
            enable_ruby_lint: false,
            enable_yaml_lint: false,
            fail_on_warnings: false,
            fail_on_errors: true,
        }
    }
}

/// Comprehensive validation report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub module_path: String,
    pub timestamp: String,
    pub overall_status: ValidationStatus,
    pub results: Vec<LintResult>,
    pub total_issues: usize,
    pub errors: usize,
    pub warnings: usize,
    pub info_count: usize,
    pub auto_fixed: usize,
}

/// Validation status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationStatus {
    Success,
    Warnings,
    Errors,
    Failed,
}

impl ModuleValidator {
    /// Create a new module validator
    pub fn new(module_path: impl AsRef<Path>, config: ValidatorConfig) -> Result<Self> {
        let path = module_path.as_ref().to_path_buf();
        if !path.exists() {
            anyhow::bail!("Module path does not exist: {:?}", path);
        }

        Ok(Self {
            module_path: path,
            config,
        })
    }

    /// Validate entire module with all enabled validators
    pub fn validate_all(&self) -> Result<ValidationReport> {
        let mut results = Vec::new();
        let mut total_issues = 0;
        let mut errors = 0;
        let mut warnings = 0;
        let mut info_count = 0;

        // Run puppet-lint validation
        if self.config.enable_puppet_lint {
            if let Ok(result) = self.validate_puppet_lint() {
                total_issues += result.issues.len();
                errors += result.issues.iter().filter(|i| i.level == LintLevel::Error).count();
                warnings += result.issues.iter().filter(|i| i.level == LintLevel::Warning).count();
                info_count += result.issues.iter().filter(|i| i.level == LintLevel::Info).count();
                results.push(result);
            }
        }

        // Run puppet-syntax validation
        if self.config.enable_puppet_syntax {
            if let Ok(result) = self.validate_puppet_syntax() {
                total_issues += result.issues.len();
                errors += result.issues.iter().filter(|i| i.level == LintLevel::Error).count();
                warnings += result.issues.iter().filter(|i| i.level == LintLevel::Warning).count();
                info_count += result.issues.iter().filter(|i| i.level == LintLevel::Info).count();
                results.push(result);
            }
        }

        // Run metadata validation
        if self.config.enable_metadata_lint {
            if let Ok(result) = self.validate_metadata() {
                total_issues += result.issues.len();
                errors += result.issues.iter().filter(|i| i.level == LintLevel::Error).count();
                warnings += result.issues.iter().filter(|i| i.level == LintLevel::Warning).count();
                info_count += result.issues.iter().filter(|i| i.level == LintLevel::Info).count();
                results.push(result);
            }
        }

        // Run Ruby validation
        if self.config.enable_ruby_lint {
            if let Ok(result) = self.validate_ruby() {
                total_issues += result.issues.len();
                errors += result.issues.iter().filter(|i| i.level == LintLevel::Error).count();
                warnings += result.issues.iter().filter(|i| i.level == LintLevel::Warning).count();
                info_count += result.issues.iter().filter(|i| i.level == LintLevel::Info).count();
                results.push(result);
            }
        }

        // Run YAML validation
        if self.config.enable_yaml_lint {
            if let Ok(result) = self.validate_yaml() {
                total_issues += result.issues.len();
                errors += result.issues.iter().filter(|i| i.level == LintLevel::Error).count();
                warnings += result.issues.iter().filter(|i| i.level == LintLevel::Warning).count();
                info_count += result.issues.iter().filter(|i| i.level == LintLevel::Info).count();
                results.push(result);
            }
        }

        let overall_status = if errors > 0 {
            ValidationStatus::Errors
        } else if warnings > 0 {
            ValidationStatus::Warnings
        } else {
            ValidationStatus::Success
        };

        Ok(ValidationReport {
            module_path: self.module_path.to_string_lossy().to_string(),
            timestamp: chrono::Local::now().to_rfc3339(),
            overall_status,
            results,
            total_issues,
            errors,
            warnings,
            info_count,
            auto_fixed: 0,
        })
    }

    /// Validate only Puppet lint
    pub fn validate_puppet_lint(&self) -> Result<LintResult> {
        let validator = PuppetValidator::new(&self.module_path)?;
        validator.lint()
    }

    /// Validate Puppet syntax
    pub fn validate_puppet_syntax(&self) -> Result<LintResult> {
        let validator = PuppetValidator::new(&self.module_path)?;
        validator.check_syntax()
    }

    /// Validate metadata
    pub fn validate_metadata(&self) -> Result<LintResult> {
        let validator = MetadataValidator::new(&self.module_path)?;
        validator.validate()
    }

    /// Validate Ruby code
    pub fn validate_ruby(&self) -> Result<LintResult> {
        let validator = RubyValidator::new(&self.module_path)?;
        validator.lint()
    }

    /// Validate YAML files
    pub fn validate_yaml(&self) -> Result<LintResult> {
        let validator = YamlValidator::new(&self.module_path)?;
        validator.validate()
    }

    /// Get configuration
    pub fn config(&self) -> &ValidatorConfig {
        &self.config
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
    fn test_validator_config_defaults() {
        let config = ValidatorConfig::default();
        assert!(config.enable_puppet_lint);
        assert!(config.enable_puppet_syntax);
        assert!(config.enable_metadata_lint);
        assert!(!config.enable_ruby_lint);
        assert!(!config.enable_yaml_lint);
        assert!(!config.fail_on_warnings);
        assert!(config.fail_on_errors);
    }

    #[test]
    fn test_validator_config_builder() {
        let config = ValidatorConfig {
            enable_puppet_lint: false,
            enable_puppet_syntax: true,
            enable_metadata_lint: true,
            enable_ruby_lint: true,
            enable_yaml_lint: true,
            fail_on_warnings: true,
            fail_on_errors: true,
        };

        assert!(!config.enable_puppet_lint);
        assert!(config.enable_puppet_syntax);
        assert!(config.enable_metadata_lint);
        assert!(config.enable_ruby_lint);
        assert!(config.enable_yaml_lint);
        assert!(config.fail_on_warnings);
    }

    #[test]
    fn test_validation_status_ordering() {
        assert_eq!(ValidationStatus::Success, ValidationStatus::Success);
        assert_ne!(ValidationStatus::Success, ValidationStatus::Warnings);
        assert_ne!(ValidationStatus::Warnings, ValidationStatus::Errors);
    }

    #[test]
    fn test_validator_creation() {
        let result = ModuleValidator::new(".", ValidatorConfig::default());
        assert!(result.is_ok());
    }

    #[test]
    fn test_validator_invalid_path() {
        let result = ModuleValidator::new("/nonexistent/path", ValidatorConfig::default());
        assert!(result.is_err());
    }

    #[test]
    fn test_validator_getters() {
        let validator = ModuleValidator::new(".", ValidatorConfig::default()).unwrap();
        assert!(validator.module_path().exists());
        assert!(validator.config().enable_puppet_lint);
    }
}
