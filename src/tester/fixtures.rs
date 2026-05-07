use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::collections::HashMap;

/// How a fixture module should be installed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FixtureSource {
    /// Symlink `target` (a path relative to the module root, or absolute) into `spec/fixtures/modules/<name>`.
    Symlink { target: String },
    /// `git clone <repo>` (optionally `--branch <ref_value>`) into `spec/fixtures/modules/<name>`.
    Git { repo: String, ref_value: Option<String> },
    /// Forge module reference like `puppetlabs/stdlib`. Recognised but installation is skipped
    /// (regent has no forge client); the user is expected to vendor or symlink these.
    Forge { slug: String },
}

/// Represents a fixture module dependency
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FixtureModule {
    pub name: String,
    pub source: Option<FixtureSource>,
}

impl FixtureModule {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            source: None,
        }
    }

    pub fn with_repo(mut self, repo: impl Into<String>) -> Self {
        let repo = repo.into();
        let ref_value = match &self.source {
            Some(FixtureSource::Git { ref_value, .. }) => ref_value.clone(),
            _ => None,
        };
        self.source = Some(FixtureSource::Git { repo, ref_value });
        self
    }

    pub fn with_ref(mut self, ref_value: impl Into<String>) -> Self {
        let ref_value = Some(ref_value.into());
        let repo = match &self.source {
            Some(FixtureSource::Git { repo, .. }) => repo.clone(),
            _ => String::new(),
        };
        self.source = Some(FixtureSource::Git { repo, ref_value });
        self
    }

    pub fn with_symlink(mut self, target: impl Into<String>) -> Self {
        self.source = Some(FixtureSource::Symlink { target: target.into() });
        self
    }

    pub fn with_forge(mut self, slug: impl Into<String>) -> Self {
        self.source = Some(FixtureSource::Forge { slug: slug.into() });
        self
    }

    /// Backwards-compatibility shim used by callers that read just the repo URL.
    pub fn repo(&self) -> Option<&str> {
        match &self.source {
            Some(FixtureSource::Git { repo, .. }) => Some(repo.as_str()),
            _ => None,
        }
    }

    pub fn ref_value(&self) -> Option<&str> {
        match &self.source {
            Some(FixtureSource::Git { ref_value, .. }) => ref_value.as_deref(),
            _ => None,
        }
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

    /// Parse .fixtures.yml file. Supports the standard rspec-puppet-fixtures shape:
    ///
    /// ```yaml
    /// fixtures:
    ///   symlinks:
    ///     mymodule: "#{source_dir}"
    ///   repositories:
    ///     stdlib:
    ///       repo: "https://github.com/puppetlabs/puppetlabs-stdlib.git"
    ///       ref:  "v9.4.1"
    ///     concat: "https://github.com/puppetlabs/puppetlabs-concat.git"
    ///   forge_modules:
    ///     archive: "voxpupuli/archive"
    /// ```
    pub fn parse_fixtures_yml(&mut self, fixtures_yml_path: &Path) -> Result<()> {
        if !fixtures_yml_path.exists() {
            return Err(anyhow!("fixtures.yml not found at {:?}", fixtures_yml_path));
        }

        let content = fs::read_to_string(fixtures_yml_path)
            .context("Failed to read fixtures.yml")?;

        self.config = parse_fixtures_yaml(&content)?;
        Ok(())
    }

    /// Setup fixtures by materialising each module under `spec/fixtures/modules/<name>`.
    ///
    /// - `Symlink` targets are resolved relative to `module_path` (or absolute), and a symlink
    ///   is created. On non-Unix platforms, falls back to creating a directory copy reference.
    /// - `Git` sources are fetched via `git clone` (with `--branch <ref>` if set). If `git`
    ///   is unavailable or the clone fails, a stub directory is created so loading does not
    ///   abort the rest of setup.
    /// - `Forge` and source-less entries get a stub directory + minimal `metadata.json`.
    ///
    /// Returns the number of fixture modules processed (skipping ones already on disk).
    pub fn setup_fixtures(&self) -> Result<usize> {
        fs::create_dir_all(&self.fixtures_dir)
            .context("Failed to create fixtures directory")?;

        let mut count = 0;
        let Some(ref modules) = self.config.modules else {
            return Ok(0);
        };

        for (module_name, module) in modules {
            let fixture_path = self.fixtures_dir.join(module_name);
            if fixture_path.exists() || fixture_path.symlink_metadata().is_ok() {
                continue;
            }

            let installed = match &module.source {
                Some(FixtureSource::Symlink { target }) => {
                    self.install_symlink(&fixture_path, target)
                        .with_context(|| format!("symlink fixture {module_name}"))?;
                    true
                }
                Some(FixtureSource::Git { repo, ref_value }) => {
                    install_git(&fixture_path, repo, ref_value.as_deref()).unwrap_or_else(|err| {
                        eprintln!(
                            "warning: git install failed for fixture {}: {}; using stub",
                            module_name, err
                        );
                        false
                    })
                }
                _ => false,
            };

            if !installed {
                install_stub(&fixture_path, module_name)
                    .with_context(|| format!("stub fixture {module_name}"))?;
            }

            count += 1;
        }

        Ok(count)
    }

    fn install_symlink(&self, fixture_path: &Path, target: &str) -> Result<()> {
        let resolved = if Path::new(target).is_absolute() {
            PathBuf::from(target)
        } else {
            self.module_path.join(target)
        };
        if !resolved.exists() {
            return Err(anyhow!(
                "symlink target does not exist: {}",
                resolved.display()
            ));
        }
        symlink_dir(&resolved, fixture_path)
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

fn install_git(fixture_path: &Path, repo: &str, ref_value: Option<&str>) -> Result<bool> {
    if repo.is_empty() {
        return Ok(false);
    }
    let mut cmd = std::process::Command::new("git");
    cmd.arg("clone").arg("--depth").arg("1");
    if let Some(reference) = ref_value {
        cmd.arg("--branch").arg(reference);
    }
    cmd.arg(repo).arg(fixture_path);
    let status = cmd.status().context("invoke git clone")?;
    if !status.success() {
        return Err(anyhow!("git clone exited with status {}", status));
    }
    Ok(true)
}

fn install_stub(fixture_path: &Path, module_name: &str) -> Result<()> {
    fs::create_dir_all(fixture_path)?;
    let metadata_path = fixture_path.join("metadata.json");
    fs::write(
        &metadata_path,
        format!(
            r#"{{"name":"{}","version":"1.0.0","dependencies":[]}}"#,
            module_name
        ),
    )?;
    Ok(())
}

#[cfg(unix)]
fn symlink_dir(target: &Path, link: &Path) -> Result<()> {
    std::os::unix::fs::symlink(target, link).context("create symlink")
}

#[cfg(windows)]
fn symlink_dir(target: &Path, link: &Path) -> Result<()> {
    std::os::windows::fs::symlink_dir(target, link).context("create symlink")
}

/// Parse a `.fixtures.yml` document. The parser is intentionally narrow — it understands the
/// `fixtures:` top-level shape used by rspec-puppet-fixtures (`symlinks`, `repositories`,
/// `forge_modules`) and ignores anything else without erroring.
fn parse_fixtures_yaml(content: &str) -> Result<FixtureConfig> {
    let mut config = FixtureConfig::new();
    if content.trim().is_empty() {
        return Ok(config);
    }

    // Section parser: collect lines indented under `fixtures:` and walk by indentation.
    let lines: Vec<&str> = content
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect();

    let mut idx = 0;
    while idx < lines.len() {
        let line = lines[idx];
        if line.trim_start().starts_with("fixtures:") {
            idx += 1;
            // Walk children of `fixtures:` (indent > 0).
            while idx < lines.len() {
                let raw = lines[idx];
                let indent = leading_spaces(raw);
                let trimmed = raw.trim();
                if trimmed.is_empty() {
                    idx += 1;
                    continue;
                }
                if indent == 0 {
                    break;
                }
                if indent == 2 {
                    let section = trimmed.trim_end_matches(':');
                    idx += 1;
                    idx = parse_section(&lines, idx, section, &mut config);
                    continue;
                }
                idx += 1;
            }
            continue;
        }
        idx += 1;
    }

    Ok(config)
}

fn parse_section(
    lines: &[&str],
    mut idx: usize,
    section: &str,
    config: &mut FixtureConfig,
) -> usize {
    while idx < lines.len() {
        let raw = lines[idx];
        let indent = leading_spaces(raw);
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            idx += 1;
            continue;
        }
        if indent < 4 {
            break;
        }

        // Each entry sits at indent == 4: either `name: "value"` or `name:` followed by
        // a mapping at indent == 6.
        if indent == 4 {
            if let Some((key, value)) = split_kv(trimmed) {
                if value.is_empty() {
                    // Look ahead for nested mapping.
                    let mut nested_repo: Option<String> = None;
                    let mut nested_ref: Option<String> = None;
                    idx += 1;
                    while idx < lines.len() {
                        let nraw = lines[idx];
                        let nindent = leading_spaces(nraw);
                        let ntrim = nraw.trim();
                        if ntrim.is_empty() {
                            idx += 1;
                            continue;
                        }
                        if nindent < 6 {
                            break;
                        }
                        if let Some((nk, nv)) = split_kv(ntrim) {
                            match nk.as_str() {
                                "repo" => nested_repo = Some(strip_quotes(&nv).to_string()),
                                "ref" => nested_ref = Some(strip_quotes(&nv).to_string()),
                                _ => {}
                            }
                        }
                        idx += 1;
                    }
                    let module = build_fixture_module(section, &key, "", nested_repo, nested_ref);
                    insert_module(config, module);
                    continue;
                } else {
                    let value = strip_quotes(&value).to_string();
                    let module = build_fixture_module(section, &key, &value, None, None);
                    insert_module(config, module);
                    idx += 1;
                    continue;
                }
            }
        }
        idx += 1;
    }
    idx
}

fn build_fixture_module(
    section: &str,
    name: &str,
    value: &str,
    nested_repo: Option<String>,
    nested_ref: Option<String>,
) -> FixtureModule {
    let module = FixtureModule::new(name);
    match section {
        "symlinks" => module.with_symlink(value),
        "repositories" => {
            let repo = nested_repo.unwrap_or_else(|| value.to_string());
            let mut m = module.with_repo(repo);
            if let Some(r) = nested_ref {
                m = m.with_ref(r);
            }
            m
        }
        "forge_modules" => module.with_forge(value),
        _ => module,
    }
}

fn insert_module(config: &mut FixtureConfig, module: FixtureModule) {
    config
        .modules
        .get_or_insert_with(HashMap::new)
        .insert(module.name.clone(), module);
}

fn leading_spaces(line: &str) -> usize {
    line.chars().take_while(|c| *c == ' ').count()
}

fn split_kv(text: &str) -> Option<(String, String)> {
    let mut parts = text.splitn(2, ':');
    let key = parts.next()?.trim().to_string();
    let value = parts.next().unwrap_or("").trim().to_string();
    if key.is_empty() {
        return None;
    }
    Some((key, value))
}

fn strip_quotes(text: &str) -> &str {
    let trimmed = text.trim();
    if (trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2)
        || (trimmed.starts_with('\'') && trimmed.ends_with('\'') && trimmed.len() >= 2)
    {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
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
        assert_eq!(module.source, None);
    }

    #[test]
    fn test_fixture_module_with_repo() {
        let module = FixtureModule::new("test")
            .with_repo("https://github.com/test/test.git");

        assert_eq!(module.name, "test");
        assert_eq!(module.repo(), Some("https://github.com/test/test.git"));
    }

    #[test]
    fn test_fixture_module_with_ref() {
        let module = FixtureModule::new("test")
            .with_repo("https://example.invalid/test.git")
            .with_ref("v1.0.0");

        assert_eq!(module.ref_value(), Some("v1.0.0"));
    }

    #[test]
    fn test_parse_fixtures_yaml_repositories_and_symlinks() {
        let doc = r##"
fixtures:
  symlinks:
    mymod: "#{source_dir}"
  repositories:
    stdlib:
      repo: "https://github.com/puppetlabs/puppetlabs-stdlib.git"
      ref:  "v9.4.1"
    concat: "https://github.com/puppetlabs/puppetlabs-concat.git"
  forge_modules:
    archive: "voxpupuli/archive"
"##;
        let cfg = parse_fixtures_yaml(doc).unwrap();
        let modules = cfg.modules.expect("modules parsed");
        assert_eq!(modules.len(), 4);
        let stdlib = modules.get("stdlib").unwrap();
        assert_eq!(stdlib.repo(), Some("https://github.com/puppetlabs/puppetlabs-stdlib.git"));
        assert_eq!(stdlib.ref_value(), Some("v9.4.1"));
        let concat = modules.get("concat").unwrap();
        assert_eq!(concat.repo(), Some("https://github.com/puppetlabs/puppetlabs-concat.git"));
        let mymod = modules.get("mymod").unwrap();
        assert!(matches!(mymod.source, Some(FixtureSource::Symlink { .. })));
        let archive = modules.get("archive").unwrap();
        assert!(matches!(archive.source, Some(FixtureSource::Forge { .. })));
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
