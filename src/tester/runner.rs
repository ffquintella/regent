use anyhow::{Context, Result};
use colored::Colorize;
use std::process::Command;
use std::time::Instant;

use super::{CoverageReport, TestCase, TestConfig, TestResults, TestStatus};
use std::collections::HashMap;

/// Runs tests against Puppet modules
pub struct TestRunner<'a> {
    config: &'a TestConfig,
}

impl<'a> TestRunner<'a> {
    pub fn new(config: &'a TestConfig) -> Self {
        Self { config }
    }

    /// Execute RSpec-Puppet unit tests
    pub fn run_unit_tests(&self) -> Result<TestResults> {
        let ruby = self.ruby_path()?;
        let ruby_version = self.ruby_version(&ruby)?;
        println!(
            "{} Using Ruby: {} (v{})",
            "ℹ".blue(),
            ruby.display(),
            ruby_version
        );
        self.check_ruby_version(&ruby)
            .context("Ruby >= 2.7.0 is required to run module tests")?;
        self.check_rubygems_version(&ruby)
            .context("RubyGems >= 3.2.3 is required for Bundler to work reliably on this system")?;
        if !self.has_bundle_gemfile() {
            self.check_rspec_installed(&ruby)
                .context("RSpec not found. Install with: gem install rspec-puppet")?;
        }

        let start = Instant::now();
        let output = self.execute_rspec_with_bundle_install()?;
        let duration = start.elapsed().as_millis() as u64;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let mut results = TestResults::new("unit");
        results.stdout = stdout.clone();
        results.stderr = stderr.clone();
        results.exit_code = output.status.code().unwrap_or(-1);
        results.duration_ms = duration;

        // Parse output and extract test results
        self.parse_rspec_output(&stdout, &mut results)?;

        Ok(results)
    }

    /// Execute integration tests (placeholder)
    pub fn run_integration_tests(&self) -> Result<TestResults> {
        // For now, return empty results with a note
        let mut results = TestResults::new("integration");
        results.stdout = "Integration tests not yet implemented\n".to_string();
        Ok(results)
    }

    /// Execute acceptance tests (placeholder)
    pub fn run_acceptance_tests(&self) -> Result<TestResults> {
        // For now, return empty results with a note
        let mut results = TestResults::new("acceptance");
        results.stdout = "Acceptance tests not yet implemented\n".to_string();
        Ok(results)
    }

    /// Generate code coverage report
    pub fn generate_coverage(&self) -> Result<CoverageReport> {
        // Placeholder implementation
        Ok(CoverageReport {
            overall_coverage: 0.0,
            lines_covered: 0,
            lines_total: 0,
            branches_covered: 0,
            branches_total: 0,
            file_coverage: HashMap::new(),
        })
    }

    /// Check if RSpec is installed
    fn check_rspec_installed(&self, ruby: &std::path::Path) -> Result<bool> {
        let mut cmd = Command::new(&ruby);
        cmd.args(&["-S", "gem", "list", "^rspec$"]);
        self.prepend_ruby_bindir(&mut cmd, ruby);
        let output = cmd
            .output()
            .context("Failed to check if RSpec is installed")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.contains("rspec"))
    }

    fn check_rubygems_version(&self, ruby: &std::path::Path) -> Result<()> {
        let output = Command::new(ruby)
            .args(&["-e", "print Gem::VERSION"])
            .output()
            .context("Failed to check RubyGems version")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Unable to determine RubyGems version"));
        }

        let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !self.version_at_least(&version, "3.2.3") {
            return Err(anyhow::anyhow!(
                "RubyGems {} is too old. Please update: `gem update --system 3.2.3`",
                version
            ));
        }
        Ok(())
    }

    fn check_ruby_version(&self, ruby: &std::path::Path) -> Result<()> {
        let output = Command::new(ruby)
            .args(&["-e", "print RUBY_VERSION"])
            .output()
            .context("Failed to check Ruby version")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Unable to determine Ruby version"));
        }

        let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !self.version_at_least(&version, "2.7.0") {
            return Err(anyhow::anyhow!(
                "Ruby {} is too old. Please upgrade Ruby (>= 2.7.0) to run tests.",
                version
            ));
        }
        Ok(())
    }

    fn version_at_least(&self, current: &str, minimum: &str) -> bool {
        let parse = |value: &str| {
            value
                .split('.')
                .map(|part| part.parse::<u32>().unwrap_or(0))
                .collect::<Vec<u32>>()
        };
        let current_parts = parse(current);
        let minimum_parts = parse(minimum);
        for index in 0..minimum_parts.len().max(current_parts.len()) {
            let a = *current_parts.get(index).unwrap_or(&0);
            let b = *minimum_parts.get(index).unwrap_or(&0);
            if a > b {
                return true;
            }
            if a < b {
                return false;
            }
        }
        true
    }

    fn ruby_path(&self) -> Result<std::path::PathBuf> {
        if let Ok(path) = std::env::var("REGENT_RUBY") {
            let candidate = std::path::PathBuf::from(path);
            if is_executable(&candidate) {
                return Ok(candidate);
            }
        }

        if let Some(system_ruby) = find_executable("ruby") {
            if let Ok(version) = self.ruby_version(&system_ruby) {
                if self.version_at_least(&version, "2.7.0") {
                    return Ok(system_ruby);
                }
            }
        }

        if let Some(puppet_ruby) = self.find_puppet_ruby() {
            if let Ok(version) = self.ruby_version(&puppet_ruby) {
                if self.version_at_least(&version, "2.7.0") {
                    return Ok(puppet_ruby);
                }
            }
        }

        Err(anyhow::anyhow!(
            "No suitable Ruby found. Install Ruby >= 2.7.0 or set REGENT_RUBY."
        ))
    }

    fn ruby_version(&self, ruby: &std::path::Path) -> Result<String> {
        let output = Command::new(ruby)
            .args(&["-e", "print RUBY_VERSION"])
            .output()
            .context("Failed to read Ruby version")?;
        if !output.status.success() {
            return Err(anyhow::anyhow!("Unable to determine Ruby version"));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn find_puppet_ruby(&self) -> Option<std::path::PathBuf> {
        let puppet = find_executable("puppet")?;
        let puppet_dir = puppet.parent()?;
        let mut candidates = Vec::new();
        candidates.push(puppet_dir.join("ruby"));
        if let Some(root) = puppet_dir.parent() {
            candidates.push(root.join("puppet").join("bin").join("ruby"));
            candidates.push(root.join("bin").join("ruby"));
        }
        for candidate in candidates {
            if is_executable(&candidate) {
                return Some(candidate.to_path_buf());
            }
        }
        None
    }

    /// Execute the rspec command
    fn execute_rspec_command(&self) -> Result<std::process::Output> {
        let ruby = self.ruby_path()?;
        let mut cmd = if self.has_bundle_gemfile() {
            let mut bundle_cmd = Command::new(&ruby);
            bundle_cmd.arg("-S").arg("bundle").arg("exec").arg("rspec");
            bundle_cmd
                .env("BUNDLE_GEMFILE", self.config.module_path.join("Gemfile"))
                .env("BUNDLE_APP_CONFIG", self.config.module_path.join(".bundle"))
                .env(
                    "BUNDLE_PATH",
                    self.config.module_path.join("vendor").join("bundle"),
                )
                .env("BUNDLE_FORCE_RUBY_PLATFORM", "true");
            self.prepend_ruby_bindir(&mut bundle_cmd, &ruby);
            bundle_cmd
        } else {
            let mut rspec_cmd = Command::new(&ruby);
            rspec_cmd.arg("-S").arg("rspec");
            rspec_cmd
        };

        // Set working directory to module path
        cmd.current_dir(&self.config.module_path);

        // Add RSpec options
        cmd.arg("--format").arg("progress").arg("--color");

        if let Some(pattern) = &self.config.pattern {
            cmd.arg("--pattern").arg(pattern);
        }

        // Add coverage if requested
        if self.config.coverage {
            cmd.arg("--require")
                .arg("simplecov")
                .arg("--require")
                .arg("simplecov-console");
        }

        // Execute and capture output
        cmd.output().context("Failed to execute RSpec command")
    }

    fn execute_rspec_with_bundle_install(&self) -> Result<std::process::Output> {
        let output = self.execute_rspec_command()?;
        if self.should_run_bundle_install(&output) {
            println!(
                "{}",
                "Running bundle install to resolve missing gems...".yellow()
            );
            self.run_bundle_install()?;
            let retry = self.execute_rspec_command()?;
            if self.should_run_bundle_install(&retry) {
                if let Some(fallback) = self.execute_rspec_fallback() {
                    return Ok(fallback);
                }
            }
            return Ok(retry);
        }
        Ok(output)
    }

    fn execute_rspec_fallback(&self) -> Option<std::process::Output> {
        let rspec_path = self.bundle_rspec_path()?;
        let bundle_env = self.bundle_gem_env()?;

        let mut cmd = Command::new(rspec_path);
        cmd.current_dir(&self.config.module_path);
        cmd.env("GEM_HOME", &bundle_env.home);
        cmd.env("GEM_PATH", &bundle_env.path);
        cmd.arg("--format").arg("progress").arg("--color");
        if let Some(pattern) = &self.config.pattern {
            cmd.arg("--pattern").arg(pattern);
        }
        cmd.output().ok()
    }

    fn bundle_rspec_path(&self) -> Option<std::path::PathBuf> {
        let ruby_dir = self
            .config
            .module_path
            .join("vendor")
            .join("bundle")
            .join("ruby");
        let entries = std::fs::read_dir(ruby_dir).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let candidate = path.join("bin").join("rspec");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        None
    }

    fn should_run_bundle_install(&self, output: &std::process::Output) -> bool {
        if !self.has_bundle_gemfile() {
            return false;
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{stdout}\n{stderr}");
        combined.contains("Bundler::GemNotFound")
            || combined.contains("Could not find")
            || combined.contains("missing extensions")
            || combined.contains("command not found: rspec")
            || combined.contains("Install missing gem executables")
    }

    fn run_bundle_install(&self) -> Result<()> {
        let ruby = self.ruby_path()?;
        self.ensure_bundler(&ruby)?;
        let mut cmd = Command::new(&ruby);
        cmd.arg("-S")
            .arg("bundle")
            .arg("install")
            .arg("--path")
            .arg(self.config.module_path.join("vendor").join("bundle"))
            .env("BUNDLE_FORCE_RUBY_PLATFORM", "true")
            .env("BUNDLE_GEMFILE", self.config.module_path.join("Gemfile"))
            .env("BUNDLE_APP_CONFIG", self.config.module_path.join(".bundle"))
            .current_dir(&self.config.module_path);
        self.prepend_ruby_bindir(&mut cmd, &ruby);

        let output = cmd.output().context("Failed to execute bundle install")?;
        if !output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!(
                "bundle install failed:\n{}\n{}",
                stdout.trim_end(),
                stderr.trim_end()
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stdout.trim().is_empty() {
            println!("{stdout}");
        }
        if !stderr.trim().is_empty() {
            println!("{stderr}");
        }
        Ok(())
    }

    fn ensure_bundler(&self, ruby: &std::path::Path) -> Result<()> {
        let mut check_cmd = Command::new(ruby);
        check_cmd.args(&["-S", "bundle", "--version"]);
        self.prepend_ruby_bindir(&mut check_cmd, ruby);
        let check = check_cmd.output();
        if let Ok(output) = check {
            if output.status.success() {
                return Ok(());
            }
        }

        let version = self
            .bundler_version_from_lockfile()
            .unwrap_or_else(|| "2.4.21".to_string());
        let mut install_cmd = Command::new(ruby);
        install_cmd.args(&[
            "-S",
            "gem",
            "install",
            "bundler",
            "-v",
            &version,
            "--no-document",
        ]);
        self.prepend_ruby_bindir(&mut install_cmd, ruby);
        let output = install_cmd.output().context("Failed to install Bundler")?;
        if !output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!(
                "Bundler install failed:\n{}\n{}",
                stdout.trim_end(),
                stderr.trim_end()
            ));
        }
        Ok(())
    }

    fn bundler_version_from_lockfile(&self) -> Option<String> {
        let lockfile = self.config.module_path.join("Gemfile.lock");
        let content = std::fs::read_to_string(lockfile).ok()?;
        let mut lines = content.lines();
        while let Some(line) = lines.next() {
            if line.trim() == "BUNDLED WITH" {
                return lines.next().map(|value| value.trim().to_string());
            }
        }
        None
    }

    fn has_bundle_gemfile(&self) -> bool {
        self.config.module_path.join("Gemfile").exists()
    }

    /// Parse RSpec output and extract test results
    fn parse_rspec_output(&self, output: &str, results: &mut TestResults) -> Result<()> {
        // Parse the output line by line
        for line in output.lines() {
            // Look for the summary line (e.g., "5 examples, 0 failures")
            if line.contains("example") || line.contains("spec") {
                self.parse_summary_line(line, results)?;
            }

            // Look for individual test results
            if line.contains("✓") || line.contains("✗") || line.contains("FAILED") {
                self.parse_test_line(line, results)?;
            }
        }

        // If no tests were parsed, try to detect from exit code
        if results.total == 0 {
            results.total = 1;
            if results.exit_code == 0 {
                results.passed = 1;
            } else {
                results.failed = 1;
            }
        }

        Ok(())
    }

    /// Parse summary line like "5 examples, 0 failures"
    fn parse_summary_line(&self, line: &str, results: &mut TestResults) -> Result<()> {
        // Extract number of examples
        if let Some(pos) = line.find("example") {
            let before = &line[..pos];
            if let Some(num_str) = before.split_whitespace().last() {
                if let Ok(total) = num_str.parse::<usize>() {
                    results.total = total;
                }
            }
        }

        // Extract number of failures
        if let Some(pos) = line.find("failure") {
            let before = &line[..pos];
            if let Some(num_str) = before.split_whitespace().last() {
                if let Ok(failed) = num_str.parse::<usize>() {
                    results.failed = failed;
                }
            }
        }

        // Calculate passed = total - failed
        if results.total > 0 && results.failed > 0 {
            results.passed = results.total - results.failed;
        } else if results.total > 0 {
            results.passed = results.total;
        }

        Ok(())
    }

    /// Parse individual test line
    fn parse_test_line(&self, line: &str, results: &mut TestResults) -> Result<()> {
        let trimmed = line.trim();

        // Extract test name and status
        let (name, status) = if trimmed.contains("✓") || trimmed.starts_with(".") {
            (trimmed.to_string(), TestStatus::Passed)
        } else if trimmed.contains("✗") || trimmed.starts_with("F") {
            (trimmed.to_string(), TestStatus::Failed)
        } else if trimmed.contains("pending") || trimmed.contains("skip") {
            (trimmed.to_string(), TestStatus::Skipped)
        } else {
            return Ok(());
        };

        // Create test case
        let test_case = TestCase {
            name,
            status,
            duration_ms: 0,
            message: None,
        };

        // Only add if it looks like a real test result
        if !trimmed.is_empty() && trimmed.len() > 2 {
            results.add_test_case(test_case);
        }

        Ok(())
    }

    fn bundle_gem_env(&self) -> Option<BundleGemEnv> {
        let bundle_path = self
            .config
            .module_path
            .join("vendor")
            .join("bundle")
            .join("ruby");
        let entries = std::fs::read_dir(bundle_path).ok()?;
        let bundle_ruby = entries
            .flatten()
            .map(|entry| entry.path())
            .find(|path| path.is_dir())?;

        let gem_path = bundle_ruby.to_string_lossy().to_string();

        Some(BundleGemEnv {
            home: bundle_ruby,
            path: gem_path,
        })
    }

    fn prepend_ruby_bindir(&self, cmd: &mut Command, ruby: &std::path::Path) {
        if let Some(bindir) = self.ruby_bindir(ruby) {
            let existing = std::env::var("PATH").unwrap_or_default();
            let updated = format!("{}:{}", bindir.display(), existing);
            cmd.env("PATH", updated);
        }
    }

    fn ruby_bindir(&self, ruby: &std::path::Path) -> Option<std::path::PathBuf> {
        let output = Command::new(ruby)
            .args(&["-e", "print Gem.bindir"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        Some(std::path::PathBuf::from(
            String::from_utf8_lossy(&output.stdout).to_string(),
        ))
    }
}

struct BundleGemEnv {
    home: std::path::PathBuf,
    path: String,
}

fn find_executable(command: &str) -> Option<std::path::PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join(command))
            .find(|candidate| is_executable(candidate))
    })
}

fn is_executable(path: &std::path::Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(windows)]
    {
        return true;
    }
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = path.metadata() {
            return metadata.permissions().mode() & 0o111 != 0;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runner_creation() {
        let config = TestConfig::new("/tmp/module", crate::tester::TestType::Unit);
        let runner = TestRunner::new(&config);
        assert_eq!(runner.config.test_type, crate::tester::TestType::Unit);
    }

    #[test]
    fn test_parse_summary_line_examples() {
        let config = TestConfig::new("/tmp/module", crate::tester::TestType::Unit);
        let runner = TestRunner::new(&config);
        let mut results = TestResults::new("unit");

        runner
            .parse_summary_line("5 examples, 0 failures", &mut results)
            .unwrap();

        assert_eq!(results.total, 5);
        assert_eq!(results.passed, 5);
        assert_eq!(results.failed, 0);
    }

    #[test]
    fn test_parse_summary_line_with_failures() {
        let config = TestConfig::new("/tmp/module", crate::tester::TestType::Unit);
        let runner = TestRunner::new(&config);
        let mut results = TestResults::new("unit");

        runner
            .parse_summary_line("10 examples, 2 failures", &mut results)
            .unwrap();

        assert_eq!(results.total, 10);
        assert_eq!(results.failed, 2);
        assert_eq!(results.passed, 8);
    }

    #[test]
    fn test_parse_rspec_output_simple() {
        let config = TestConfig::new("/tmp/module", crate::tester::TestType::Unit);
        let runner = TestRunner::new(&config);
        let mut results = TestResults::new("unit");

        let output = ".....\n\n5 examples, 0 failures\n";
        runner.parse_rspec_output(output, &mut results).unwrap();

        assert_eq!(results.total, 5);
        assert_eq!(results.passed, 5);
        assert_eq!(results.failed, 0);
    }

    #[test]
    fn test_parse_rspec_output_with_failures() {
        let config = TestConfig::new("/tmp/module", crate::tester::TestType::Unit);
        let runner = TestRunner::new(&config);
        let mut results = TestResults::new("unit");

        let output = "..F.F\n\n5 examples, 2 failures\n";
        runner.parse_rspec_output(output, &mut results).unwrap();

        assert_eq!(results.total, 5);
        assert_eq!(results.failed, 2);
        assert_eq!(results.passed, 3);
    }

    #[test]
    fn test_parse_empty_output() {
        let config = TestConfig::new("/tmp/module", crate::tester::TestType::Unit);
        let runner = TestRunner::new(&config);
        let mut results = TestResults::new("unit");

        // Simulate failed execution with no output
        results.exit_code = 1;
        runner.parse_rspec_output("", &mut results).unwrap();

        // Should have at least one test case (failure)
        assert!(results.total > 0);
    }

    #[test]
    fn test_integration_tests_placeholder() {
        let config = TestConfig::new("/tmp/module", crate::tester::TestType::Integration);
        let runner = TestRunner::new(&config);

        let results = runner.run_integration_tests().unwrap();
        assert_eq!(results.test_type, "integration");
        assert!(results.stdout.contains("not yet implemented"));
    }

    #[test]
    fn test_acceptance_tests_placeholder() {
        let config = TestConfig::new("/tmp/module", crate::tester::TestType::Acceptance);
        let runner = TestRunner::new(&config);

        let results = runner.run_acceptance_tests().unwrap();
        assert_eq!(results.test_type, "acceptance");
        assert!(results.stdout.contains("not yet implemented"));
    }

    #[test]
    fn test_coverage_report_generation() {
        let config = TestConfig::new("/tmp/module", crate::tester::TestType::Unit);
        let runner = TestRunner::new(&config);

        let coverage = runner.generate_coverage().unwrap();
        assert_eq!(coverage.overall_coverage, 0.0);
    }
}
