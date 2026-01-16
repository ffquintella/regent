use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::collections::HashMap;

/// Represents a fixture module dependency
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FixtureModule {
    pub name: String,
    pub repo: Option<String>,
    pub ref_value: Option<String>,
}

impl FixtureModule {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            repo: None,
            ref_value: None,
        }
    }

    pub fn with_repo(mut self, repo: impl Into<String>) -> Self {
        self.repo = Some(repo.into());
        self
    }

    pub fn with_ref(mut self, ref_value: impl Into<String>) -> Self {
        self.ref_value = Some(ref_value.into());
        self
    }
}

/// Fixture configuration from .fixtures.yml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureConfig {
    pub fixtures: Option<String>,
    pub repositories: Option<HashMap<String, String>>,
    pub projects: Option<HashMap<String, String>>,
    pub modules: Option<HashMap<String, FixtureModule>>,
}

impl FixtureConfig {
    pub fn new() -> Self {
        Self {
            fixtures: None,
            repositories: None,
            projects: None,
            modules: None,
        }
    }

    pub fn with_fixtures(mut self, path: impl Into<String>) -> Self {
        self.fixtures = Some(path.into());
        self
    }

    pub fn add_module(mut self, name: impl Into<String>, module: FixtureModule) -> Self {
        if self.modules.is_none() {
            self.modules = Some(HashMap::new());
        }
        self.modules.as_mut().unwrap().insert(name.into(), module);
        self
    }
}

impl Default for FixtureConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Manages test fixtures and dependencies
pub struct FixtureManager {
    pub fixtures_dir: PathBuf,
    pub module_path: PathBuf,
    pub config: FixtureConfig,
}

impl FixtureManager {
    /// Create a new FixtureManager
    pub fn new(module_path: impl AsRef<Path>, fixtures_dir: impl AsRef<Path>) -> Self {
        Self {
            fixtures_dir: fixtures_dir.as_ref().to_path_buf(),
            module_path: module_path.as_ref().to_path_buf(),
            config: FixtureConfig::new(),
        }
    }

    /// Parse .fixtures.yml file
    pub fn parse_fixtures_yml(&mut self, fixtures_yml_path: &Path) -> Result<()> {
        if !fixtures_yml_path.exists() {
            return Err(anyhow!("fixtures.yml not found at {:?}", fixtures_yml_path));
        }

        let content = fs::read_to_string(fixtures_yml_path)
            .context("Failed to read fixtures.yml")?;

        // Simple YAML parsing - for production would use proper yaml parser
        // For now, just validate the file exists and is readable
        if content.trim().is_empty() {
            self.config = FixtureConfig::new();
        } else {
            // Parse fixtures: line if present
            for line in content.lines() {
                if line.trim().starts_with("fixtures:") {
                    let parts: Vec<&str> = line.split(':').collect();
                    if parts.len() > 1 {
                        self.config.fixtures = Some(parts[1].trim().to_string());
                    }
                }
                // Parse module entries like "module_name:" under modules:
                if line.contains("modules:") {
                    // Extract modules section
                    let mut in_modules = false;
                    for module_line in content.lines().skip_while(|l| !l.contains("modules:")) {
                        if module_line.contains("modules:") {
                            in_modules = true;
                            continue;
                        }
                        if in_modules && module_line.starts_with("  ") && !module_line.starts_with("    ") {
                            let module_name = module_line.trim().trim_end_matches(':').to_string();
                            if !module_name.is_empty() && self.config.modules.is_none() {
                                self.config.modules = Some(HashMap::new());
                            }
                            if let Some(ref mut modules) = self.config.modules {
                                modules.insert(module_name.clone(), FixtureModule::new(module_name));
                            }
                        }
                        if in_modules && !module_line.starts_with("  ") && !module_line.is_empty() {
                            in_modules = false;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Setup fixtures (create symlinks)
    pub fn setup_fixtures(&self) -> Result<usize> {
        fs::create_dir_all(&self.fixtures_dir)
            .context("Failed to create fixtures directory")?;

        let mut count = 0;

        if let Some(ref modules) = self.config.modules {
            for (module_name, _module) in modules {
                let fixture_path = self.fixtures_dir.join(module_name);

                // Create fixture symlink or directory
                if !fixture_path.exists() {
                    // For testing, just create a directory
                    fs::create_dir_all(&fixture_path)
                        .context(format!("Failed to setup fixture: {}", module_name))?;
                    
                    // Create a metadata.json stub
                    let metadata_path = fixture_path.join("metadata.json");
                    fs::write(
                        &metadata_path,
                        format!(
                            r#"{{"name":"{}","version":"1.0.0","dependencies":[]}}"#,
                            module_name
                        ),
                    )?;

                    count += 1;
                }
            }
        }

        Ok(count)
    }

    /// Verify fixtures are properly set up
    pub fn verify(&self) -> Result<bool> {
        if !self.fixtures_dir.exists() {
            return Ok(false);
        }

        if let Some(ref modules) = self.config.modules {
            for (module_name, _module) in modules {
                let fixture_path = self.fixtures_dir.join(module_name);
                if !fixture_path.exists() {
                    return Ok(false);
                }

                // Check for metadata.json
                let metadata_path = fixture_path.join("metadata.json");
                if !metadata_path.exists() {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    /// Clean up fixtures
    pub fn cleanup(&self) -> Result<usize> {
        let mut count = 0;

        if let Some(ref modules) = self.config.modules {
            for (module_name, _module) in modules {
                let fixture_path = self.fixtures_dir.join(module_name);

                if fixture_path.exists() {
                    fs::remove_dir_all(&fixture_path)
                        .context(format!("Failed to cleanup fixture: {}", module_name))?;
                    count += 1;
                }
            }
        }

        // Remove fixtures directory if empty
        if self.fixtures_dir.exists() {
            if let Ok(entries) = fs::read_dir(&self.fixtures_dir) {
                if entries.count() == 0 {
                    fs::remove_dir(&self.fixtures_dir).ok();
                }
            }
        }

        Ok(count)
    }

    /// Get list of fixture modules
    pub fn get_modules(&self) -> Vec<String> {
        self.config
            .modules
            .as_ref()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Get a specific fixture module
    pub fn get_module(&self, name: &str) -> Option<&FixtureModule> {
        self.config.modules.as_ref().and_then(|m| m.get(name))
    }

    /// Check if module has fixtures
    pub fn has_fixtures(&self) -> bool {
        self.config
            .modules
            .as_ref()
            .map(|m| !m.is_empty())
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_fixture_module_creation() {
        let module = FixtureModule::new("test_module");
        assert_eq!(module.name, "test_module");
        assert_eq!(module.repo, None);
        assert_eq!(module.ref_value, None);
    }

    #[test]
    fn test_fixture_module_with_repo() {
        let module = FixtureModule::new("test")
            .with_repo("https://github.com/test/test.git");
        
        assert_eq!(module.name, "test");
        assert_eq!(module.repo, Some("https://github.com/test/test.git".to_string()));
    }

    #[test]
    fn test_fixture_module_with_ref() {
        let module = FixtureModule::new("test")
            .with_ref("v1.0.0");
        
        assert_eq!(module.ref_value, Some("v1.0.0".to_string()));
    }

    #[test]
    fn test_fixture_config_creation() {
        let config = FixtureConfig::new();
        assert_eq!(config.fixtures, None);
        assert_eq!(config.modules, None);
    }

    #[test]
    fn test_fixture_config_builder() {
        let config = FixtureConfig::new()
            .with_fixtures("spec/fixtures")
            .add_module("puppet", FixtureModule::new("puppet"));

        assert_eq!(config.fixtures, Some("spec/fixtures".to_string()));
        assert_eq!(config.modules.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_fixture_manager_creation() {
        let manager = FixtureManager::new("/tmp/module", "/tmp/fixtures");
        assert_eq!(manager.module_path, PathBuf::from("/tmp/module"));
        assert_eq!(manager.fixtures_dir, PathBuf::from("/tmp/fixtures"));
    }

    #[test]
    fn test_fixture_manager_setup() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let fixtures_dir = temp_dir.path().join("fixtures");
        let module_path = temp_dir.path().join("module");

        let mut manager = FixtureManager::new(&module_path, &fixtures_dir);
        manager.config = FixtureConfig::new()
            .add_module("puppet", FixtureModule::new("puppet"))
            .add_module("stdlib", FixtureModule::new("stdlib"));

        let count = manager.setup_fixtures()?;
        assert_eq!(count, 2);
        assert!(fixtures_dir.exists());
        assert!(fixtures_dir.join("puppet").exists());
        assert!(fixtures_dir.join("stdlib").exists());

        Ok(())
    }

    #[test]
    fn test_fixture_manager_verify() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let fixtures_dir = temp_dir.path().join("fixtures");
        let module_path = temp_dir.path().join("module");

        let mut manager = FixtureManager::new(&module_path, &fixtures_dir);
        manager.config = FixtureConfig::new()
            .add_module("puppet", FixtureModule::new("puppet"));

        let verified = manager.verify()?;
        assert!(!verified);

        manager.setup_fixtures()?;
        let verified = manager.verify()?;
        assert!(verified);

        Ok(())
    }

    #[test]
    fn test_fixture_manager_cleanup() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let fixtures_dir = temp_dir.path().join("fixtures");
        let module_path = temp_dir.path().join("module");

        let mut manager = FixtureManager::new(&module_path, &fixtures_dir);
        manager.config = FixtureConfig::new()
            .add_module("puppet", FixtureModule::new("puppet"));

        manager.setup_fixtures()?;
        assert!(fixtures_dir.join("puppet").exists());

        let count = manager.cleanup()?;
        assert_eq!(count, 1);
        assert!(!fixtures_dir.join("puppet").exists());

        Ok(())
    }

    #[test]
    fn test_fixture_manager_get_modules() {
        let manager = FixtureManager::new("/tmp/module", "/tmp/fixtures");
        assert_eq!(manager.get_modules().len(), 0);

        let mut manager = FixtureManager::new("/tmp/module", "/tmp/fixtures");
        manager.config = FixtureConfig::new()
            .add_module("puppet", FixtureModule::new("puppet"))
            .add_module("stdlib", FixtureModule::new("stdlib"));

        let modules = manager.get_modules();
        assert_eq!(modules.len(), 2);
        assert!(modules.contains(&"puppet".to_string()));
        assert!(modules.contains(&"stdlib".to_string()));
    }

    #[test]
    fn test_fixture_manager_get_module() {
        let mut manager = FixtureManager::new("/tmp/module", "/tmp/fixtures");
        manager.config = FixtureConfig::new()
            .add_module("puppet", FixtureModule::new("puppet"));

        let module = manager.get_module("puppet");
        assert!(module.is_some());
        assert_eq!(module.unwrap().name, "puppet");

        let module = manager.get_module("nonexistent");
        assert!(module.is_none());
    }

    #[test]
    fn test_fixture_manager_has_fixtures() {
        let manager = FixtureManager::new("/tmp/module", "/tmp/fixtures");
        assert!(!manager.has_fixtures());

        let mut manager = FixtureManager::new("/tmp/module", "/tmp/fixtures");
        manager.config = FixtureConfig::new()
            .add_module("puppet", FixtureModule::new("puppet"));

        assert!(manager.has_fixtures());
    }

    #[test]
    fn test_fixture_manager_parse_empty_fixtures_yml() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let fixtures_yml = temp_dir.path().join("fixtures.yml");
        fs::write(&fixtures_yml, "")?;

        let mut manager = FixtureManager::new(temp_dir.path(), temp_dir.path());
        manager.parse_fixtures_yml(&fixtures_yml)?;

        assert!(manager.config.modules.is_none() || manager.config.modules.as_ref().unwrap().is_empty());
        Ok(())
    }

    #[test]
    fn test_fixture_manager_metadata_creation() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let fixtures_dir = temp_dir.path().join("fixtures");

        let mut manager = FixtureManager::new(temp_dir.path(), &fixtures_dir);
        manager.config = FixtureConfig::new()
            .add_module("test", FixtureModule::new("test"));

        manager.setup_fixtures()?;

        let metadata_path = fixtures_dir.join("test").join("metadata.json");
        assert!(metadata_path.exists());

        let content = fs::read_to_string(&metadata_path)?;
        assert!(content.contains("test"));
        assert!(content.contains("1.0.0"));

        Ok(())
    }
}
