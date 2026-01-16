//! Puppet validator - Validates Puppet code and syntax

use anyhow::Result;
use std::path::{Path, PathBuf};
use crate::validator::lint::{LintResult, LintTool};

/// Puppet code validator
pub struct PuppetValidator {
    module_path: PathBuf,
}

impl PuppetValidator {
    /// Create a new Puppet validator
    pub fn new(module_path: impl AsRef<Path>) -> Result<Self> {
        let path = module_path.as_ref().to_path_buf();
        if !path.exists() {
            anyhow::bail!("Module path does not exist: {:?}", path);
        }

        Ok(Self {
            module_path: path,
        })
    }

    /// Run puppet-lint checks
    pub fn lint(&self) -> Result<LintResult> {
        let mut result = LintResult::new(LintTool::PuppetLint);
        let start = std::time::Instant::now();

        // In a real implementation, this would invoke puppet-lint
        // For now, we'll return a template result that can be integrated
        let manifests_dir = self.module_path.join("manifests");
        if manifests_dir.exists() {
            // Scan for .pp files
            self.scan_puppet_files(&manifests_dir, &mut result)?;
        }

        result.execution_time_ms = start.elapsed().as_millis();
        result.success = result.error_count() == 0;

        Ok(result)
    }

    /// Check Puppet syntax
    pub fn check_syntax(&self) -> Result<LintResult> {
        let mut result = LintResult::new(LintTool::PuppetSyntax);
        let start = std::time::Instant::now();

        // In a real implementation, this would invoke puppet-syntax
        // For now, we'll return a template result
        let manifests_dir = self.module_path.join("manifests");
        if manifests_dir.exists() {
            self.check_puppet_syntax(&manifests_dir, &mut result)?;
        }

        result.execution_time_ms = start.elapsed().as_millis();
        result.success = result.error_count() == 0;

        Ok(result)
    }

    fn scan_puppet_files(&self, dir: &Path, _result: &mut LintResult) -> Result<()> {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "pp").unwrap_or(false) {
                    // Placeholder for actual linting logic
                }
            }
        }
        Ok(())
    }

    fn check_puppet_syntax(&self, dir: &Path, _result: &mut LintResult) -> Result<()> {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "pp").unwrap_or(false) {
                    // Placeholder for actual syntax checking
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
    fn test_puppet_validator_creation() {
        let result = PuppetValidator::new(".");
        assert!(result.is_ok());
    }

    #[test]
    fn test_puppet_validator_invalid_path() {
        let result = PuppetValidator::new("/nonexistent/puppet/path");
        assert!(result.is_err());
    }

    #[test]
    fn test_puppet_lint_result() {
        let validator = PuppetValidator::new(".").unwrap();
        let result = validator.lint();
        assert!(result.is_ok());
        let lint_result = result.unwrap();
        assert_eq!(lint_result.tool, LintTool::PuppetLint);
    }

    #[test]
    fn test_puppet_syntax_result() {
        let validator = PuppetValidator::new(".").unwrap();
        let result = validator.check_syntax();
        assert!(result.is_ok());
        let syntax_result = result.unwrap();
        assert_eq!(syntax_result.tool, LintTool::PuppetSyntax);
    }

    #[test]
    fn test_puppet_validator_getters() {
        let validator = PuppetValidator::new(".").unwrap();
        assert!(validator.module_path().exists());
    }
}
