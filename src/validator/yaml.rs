//! YAML validator - Validates YAML files in the module

use crate::validator::lint::{LintResult, LintTool};
use anyhow::Result;
use std::path::{Path, PathBuf};

/// YAML file validator
pub struct YamlValidator {
    module_path: PathBuf,
}

impl YamlValidator {
    /// Create a new YAML validator
    pub fn new(module_path: impl AsRef<Path>) -> Result<Self> {
        let path = module_path.as_ref().to_path_buf();
        if !path.exists() {
            anyhow::bail!("Module path does not exist: {:?}", path);
        }

        Ok(Self { module_path: path })
    }

    /// Validate YAML files
    pub fn validate(&self) -> Result<LintResult> {
        let mut result = LintResult::new(LintTool::YamlLint);
        let start = std::time::Instant::now();

        // In a real implementation, this would check YAML syntax
        // For now, we'll return a template result
        self.check_yaml_files(&mut result)?;

        result.execution_time_ms = start.elapsed().as_millis();
        result.success = result.error_count() == 0;

        Ok(result)
    }

    fn check_yaml_files(&self, _result: &mut LintResult) -> Result<()> {
        let yaml_dirs = vec!["hiera", "data", ".github"];

        for dir_name in yaml_dirs {
            let dir = self.module_path.join(dir_name);
            if dir.exists() {
                self.scan_yaml_files(&dir, _result)?;
            }
        }

        // Check root level YAML files
        if let Ok(entries) = std::fs::read_dir(&self.module_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file()
                    && path
                        .extension()
                        .map(|e| e == "yml" || e == "yaml")
                        .unwrap_or(false)
                {
                    // Placeholder for YAML validation
                }
            }
        }

        Ok(())
    }

    fn scan_yaml_files(&self, dir: &Path, _result: &mut LintResult) -> Result<()> {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file()
                    && path
                        .extension()
                        .map(|e| e == "yml" || e == "yaml")
                        .unwrap_or(false)
                {
                    // Placeholder for actual YAML validation
                } else if path.is_dir() {
                    self.scan_yaml_files(&path, _result)?;
                }
            }
        }
        Ok(())
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
    fn test_yaml_validator_creation() {
        let result = YamlValidator::new(".");
        assert!(result.is_ok());
    }

    #[test]
    fn test_yaml_validator_invalid_path() {
        let result = YamlValidator::new("/nonexistent/yaml/path");
        assert!(result.is_err());
    }

    #[test]
    fn test_yaml_validate_result() {
        let validator = YamlValidator::new(".").unwrap();
        let result = validator.validate();
        assert!(result.is_ok());
        let validation = result.unwrap();
        assert_eq!(validation.tool, LintTool::YamlLint);
    }

    #[test]
    fn test_yaml_validator_getters() {
        let validator = YamlValidator::new(".").unwrap();
        assert!(validator.module_path().exists());
    }
}
