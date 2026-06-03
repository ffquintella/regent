//! Ruby validator - Validates Ruby code in the module

use crate::validator::lint::{LintResult, LintTool};
use anyhow::Result;
use std::path::{Path, PathBuf};

/// Ruby code validator
pub struct RubyValidator {
    module_path: PathBuf,
}

impl RubyValidator {
    /// Create a new Ruby validator
    pub fn new(module_path: impl AsRef<Path>) -> Result<Self> {
        let path = module_path.as_ref().to_path_buf();
        if !path.exists() {
            anyhow::bail!("Module path does not exist: {:?}", path);
        }

        Ok(Self { module_path: path })
    }

    /// Run Ruby linting checks
    pub fn lint(&self) -> Result<LintResult> {
        let mut result = LintResult::new(LintTool::RubyLint);
        let start = std::time::Instant::now();

        // In a real implementation, this would invoke rubocop or similar
        // For now, we'll return a template result
        self.check_ruby_files(&mut result)?;

        result.execution_time_ms = start.elapsed().as_millis();
        result.success = result.error_count() == 0;

        Ok(result)
    }

    fn check_ruby_files(&self, result: &mut LintResult) -> Result<()> {
        let dirs_to_check = vec!["lib", "spec", "tasks"];

        for dir_name in dirs_to_check {
            let dir = self.module_path.join(dir_name);
            if dir.exists() {
                self.scan_ruby_files(&dir, result)?;
            }
        }

        Ok(())
    }

    fn scan_ruby_files(&self, dir: &Path, _result: &mut LintResult) -> Result<()> {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().map(|e| e == "rb").unwrap_or(false) {
                    // Placeholder for actual Ruby linting
                } else if path.is_dir() {
                    // Recursively check subdirectories
                    self.scan_ruby_files(&path, _result)?;
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
    fn test_ruby_validator_creation() {
        let result = RubyValidator::new(".");
        assert!(result.is_ok());
    }

    #[test]
    fn test_ruby_validator_invalid_path() {
        let result = RubyValidator::new("/nonexistent/ruby/path");
        assert!(result.is_err());
    }

    #[test]
    fn test_ruby_lint_result() {
        let validator = RubyValidator::new(".").unwrap();
        let result = validator.lint();
        assert!(result.is_ok());
        let lint_result = result.unwrap();
        assert_eq!(lint_result.tool, LintTool::RubyLint);
    }

    #[test]
    fn test_ruby_validator_getters() {
        let validator = RubyValidator::new(".").unwrap();
        assert!(validator.module_path().exists());
    }
}
