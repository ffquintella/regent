use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// How a fixture module should be installed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FixtureSource {
    /// Symlink `target` (a path relative to the module root, or absolute) into `spec/fixtures/modules/<name>`.
    Symlink { target: String },
    /// `git clone <repo>` (optionally `--branch <ref_value>`) into `spec/fixtures/modules/<name>`.
    Git {
        repo: String,
        ref_value: Option<String>,
    },
    /// Forge module reference like `puppetlabs/stdlib` (optionally
    /// `author/module:version`). Downloaded from the Puppet Forge and cached
    /// per-user (`~/.regent/fixtures`) for offline reuse.
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
        self.source = Some(FixtureSource::Symlink {
            target: target.into(),
        });
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
    /// When true, never reach the network: install only from the per-user
    /// fixture cache (falling back to a stub for anything not cached).
    offline: bool,
    /// Override for the per-user fixture cache directory. `None` uses the
    /// default (`~/.regent/fixtures`). A cache of `None` disables caching.
    cache_dir: Option<PathBuf>,
}

impl FixtureManager {
    /// Create a new FixtureManager
    pub fn new(module_path: impl AsRef<Path>, fixtures_dir: impl AsRef<Path>) -> Self {
        Self {
            fixtures_dir: fixtures_dir.as_ref().to_path_buf(),
            module_path: module_path.as_ref().to_path_buf(),
            config: FixtureConfig::new(),
            offline: false,
            cache_dir: crate::tester::bundled_gems::user_fixtures_dir(),
        }
    }

    /// Install fixtures only from the per-user cache, never touching the
    /// network. Anything not already cached falls back to a stub.
    pub fn set_offline(&mut self, offline: bool) -> &mut Self {
        self.offline = offline;
        self
    }

    /// Override the per-user fixture cache directory (mainly for tests). Pass
    /// `None` to disable caching entirely.
    pub fn set_cache_dir(&mut self, dir: Option<PathBuf>) -> &mut Self {
        self.cache_dir = dir;
        self
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

        let content =
            fs::read_to_string(fixtures_yml_path).context("Failed to read fixtures.yml")?;

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
        fs::create_dir_all(&self.fixtures_dir).context("Failed to create fixtures directory")?;

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
                    match self.install_symlink(&fixture_path, target) {
                        Ok(()) => true,
                        Err(err) => {
                            eprintln!(
                                "warning: symlink failed for fixture {}: {}; using stub",
                                module_name, err
                            );
                            false
                        }
                    }
                }
                // Forge and git fixtures route through the per-user cache: a
                // cache hit copies the module with no network, a miss downloads
                // once and populates the cache for next time. In offline mode a
                // miss falls back to a stub instead of reaching the network.
                Some(source @ FixtureSource::Git { .. })
                | Some(source @ FixtureSource::Forge { .. }) => self
                    .install_cached(&fixture_path, source)
                    .unwrap_or_else(|err| {
                        eprintln!(
                            "warning: install failed for fixture {}: {}; using stub",
                            module_name, err
                        );
                        false
                    }),
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
        let expanded = expand_fixture_target(target, &self.module_path);
        let resolved = if Path::new(&expanded).is_absolute() {
            PathBuf::from(&expanded)
        } else {
            self.module_path.join(&expanded)
        };
        if !resolved.exists() {
            return Err(anyhow!(
                "symlink target does not exist: {}",
                resolved.display()
            ));
        }
        symlink_dir(&resolved, fixture_path)
    }

    /// Install a Forge/git fixture via the per-user cache.
    ///
    /// 1. If the cache holds a valid copy for this source, copy it into
    ///    `fixture_path` (no network).
    /// 2. Otherwise, when online, download/clone into the cache, then copy it
    ///    into `fixture_path`. When offline (or caching is disabled and we're
    ///    offline), report failure so the caller installs a stub.
    ///
    /// With caching disabled (`cache_dir == None`) and online, the download is
    /// written straight to `fixture_path` (legacy behavior).
    fn install_cached(&self, fixture_path: &Path, source: &FixtureSource) -> Result<bool> {
        let Some(cache_root) = self.cache_dir.as_ref() else {
            // No cache configured: download straight into the module (unless
            // offline, in which case there's nothing we can do).
            if self.offline {
                return Ok(false);
            }
            return download_source(fixture_path, source);
        };

        let key = cache_key(source);
        let cached = cache_root.join(&key);

        if !cache_entry_is_valid(&cached) {
            if self.offline {
                eprintln!(
                    "offline: fixture not in cache ({}); using stub",
                    cached.display()
                );
                return Ok(false);
            }
            // Download into a temp dir adjacent to the cache slot, then rename
            // into place so a partial download never poisons the cache.
            fs::create_dir_all(cache_root)
                .with_context(|| format!("create fixture cache {}", cache_root.display()))?;
            let staging = cache_root.join(format!("{key}.tmp"));
            let _ = fs::remove_dir_all(&staging);
            if !download_source(&staging, source)? {
                let _ = fs::remove_dir_all(&staging);
                return Ok(false);
            }
            let _ = fs::remove_dir_all(&cached);
            fs::rename(&staging, &cached).with_context(|| {
                format!("promote {} -> {}", staging.display(), cached.display())
            })?;
        }

        copy_tree(&cached, fixture_path).with_context(|| {
            format!(
                "copy cached fixture {} -> {}",
                cached.display(),
                fixture_path.display()
            )
        })?;
        Ok(true)
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

/// Expand the standard rspec-puppet-fixtures interpolations in a symlink target.
///
/// Recognised tokens (matching the Ruby `puppetlabs_spec_helper` behavior):
///   - `#{source_dir}` → module under test (canonical path)
///   - `${source_dir}` → same
///   - `.`              → module under test
fn expand_fixture_target(target: &str, module_path: &Path) -> String {
    let module_str = module_path.to_string_lossy().into_owned();
    let mut out = target.to_string();
    out = out.replace("#{source_dir}", &module_str);
    out = out.replace("${source_dir}", &module_str);
    if out.trim() == "." {
        out = module_str;
    }
    out
}

/// Download a Forge/git fixture source into `dest` (the existing, non-cached
/// install paths). Symlink sources are not handled here.
fn download_source(dest: &Path, source: &FixtureSource) -> Result<bool> {
    match source {
        FixtureSource::Git { repo, ref_value } => install_git(dest, repo, ref_value.as_deref()),
        FixtureSource::Forge { slug } => install_forge(dest, slug),
        FixtureSource::Symlink { .. } => Ok(false),
    }
}

/// A stable per-source cache key, used as a directory name under the fixture
/// cache root. Unpinned forge/git sources cache under a `-current` / `@HEAD`
/// key; pin a version/ref in `.fixtures.yml` for a stable, reproducible entry.
fn cache_key(source: &FixtureSource) -> String {
    let raw = match source {
        FixtureSource::Forge { slug } => {
            let (slug_part, version) = match slug.split_once(':') {
                Some((s, v)) if !v.is_empty() => (s, v),
                _ => (slug.as_str(), "current"),
            };
            format!("forge/{}-{}", slug_part.replace('/', "-"), version)
        }
        FixtureSource::Git { repo, ref_value } => {
            format!("git/{}@{}", repo, ref_value.as_deref().unwrap_or("HEAD"))
        }
        FixtureSource::Symlink { target } => format!("symlink/{target}"),
    };
    sanitize_cache_key(&raw)
}

/// Reduce an arbitrary source identifier to a safe single path segment.
fn sanitize_cache_key(raw: &str) -> String {
    raw.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// A cache entry is usable only if it's a directory containing a real module
/// (a `metadata.json`), which also rules out leftover empty/partial slots.
fn cache_entry_is_valid(dir: &Path) -> bool {
    dir.is_dir() && dir.join("metadata.json").is_file()
}

/// Recursively copy the contents of `src` into `dst`, creating `dst`. Symlinks
/// are followed (fixtures are plain module trees).
fn copy_tree(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst).with_context(|| format!("create {}", dst.display()))?;
    for entry in fs::read_dir(src).with_context(|| format!("read {}", src.display()))? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&from, &to)?;
        } else {
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent).ok();
            }
            fs::copy(&from, &to)
                .with_context(|| format!("copy {} -> {}", from.display(), to.display()))?;
        }
    }
    Ok(())
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

/// Download and extract a Puppet Forge module into `fixture_path`.
///
/// `slug` is a forge module identifier in either `author/module` or
/// `author-module` form. Optionally `<slug>:<version>` pins a specific
/// release; omitting the version uses the module's current release.
///
/// The implementation:
/// 1. GET `https://forgeapi.puppet.com/v3/modules/<author>-<module>`
/// 2. Parse `current_release.file_uri` (or the matching version's file_uri)
/// 3. Download the tarball at `https://forgeapi.puppet.com<file_uri>`
/// 4. Extract into `fixture_path`, stripping the leading
///    `<author>-<module>-<version>/` directory.
pub fn install_forge(fixture_path: &Path, slug: &str) -> Result<bool> {
    let (slug_part, version_pin) = match slug.split_once(':') {
        Some((s, v)) if !v.is_empty() => (s, Some(v)),
        _ => (slug, None),
    };
    let normalized = slug_part.replace('/', "-");
    if normalized.is_empty() || !normalized.contains('-') {
        return Err(anyhow!(
            "invalid forge slug `{}` (expected author/module)",
            slug
        ));
    }

    let metadata_url = format!("https://forgeapi.puppet.com/v3/modules/{normalized}");
    let metadata: serde_json::Value = ureq::get(&metadata_url)
        .timeout(std::time::Duration::from_secs(30))
        .call()
        .with_context(|| format!("GET {metadata_url}"))?
        .into_json()
        .with_context(|| format!("parse JSON from {metadata_url}"))?;

    let file_uri = forge_file_uri(&metadata, version_pin).ok_or_else(|| {
        anyhow!("forge metadata for {normalized} did not include a downloadable release")
    })?;

    let download_url = format!("https://forgeapi.puppet.com{file_uri}");
    let mut reader = ureq::get(&download_url)
        .timeout(std::time::Duration::from_secs(120))
        .call()
        .with_context(|| format!("GET {download_url}"))?
        .into_reader();

    // Buffer the tarball to a tempfile before extracting so a slow link can't
    // leave a half-extracted module directory on disk.
    let mut tarball =
        tempfile::NamedTempFile::new().context("create tempfile for forge tarball")?;
    std::io::copy(&mut reader, tarball.as_file_mut()).context("download forge tarball")?;
    tarball.as_file_mut().sync_all().ok();
    let path = tarball.path().to_path_buf();

    extract_forge_tarball(&path, fixture_path)?;
    Ok(true)
}

fn forge_file_uri<'a>(metadata: &'a serde_json::Value, version: Option<&str>) -> Option<String> {
    if let Some(version) = version {
        if let Some(releases) = metadata.get("releases").and_then(|v| v.as_array()) {
            for release in releases {
                if release.get("version").and_then(|v| v.as_str()) == Some(version) {
                    if let Some(uri) = release.get("file_uri").and_then(|v| v.as_str()) {
                        return Some(uri.to_string());
                    }
                }
            }
        }
        return None;
    }
    metadata
        .get("current_release")
        .and_then(|r| r.get("file_uri"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn extract_forge_tarball(tarball: &Path, fixture_path: &Path) -> Result<()> {
    use flate2::read::GzDecoder;
    use tar::Archive;

    let file = fs::File::open(tarball).with_context(|| format!("open {}", tarball.display()))?;
    let gz = GzDecoder::new(file);
    let mut archive = Archive::new(gz);

    fs::create_dir_all(fixture_path)
        .with_context(|| format!("create {}", fixture_path.display()))?;

    for entry in archive.entries()? {
        let mut entry = entry?;
        let raw_path = entry.path()?.into_owned();
        // Strip the leading `<author>-<module>-<version>/` directory so the
        // module contents land directly in `fixture_path/`.
        let mut components = raw_path.components();
        components.next();
        let rel: PathBuf = components.as_path().to_path_buf();
        if rel.as_os_str().is_empty() {
            continue;
        }
        let dest = fixture_path.join(&rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).ok();
        }
        entry
            .unpack(&dest)
            .with_context(|| format!("unpack {}", dest.display()))?;
    }
    Ok(())
}

fn install_stub(fixture_path: &Path, module_name: &str) -> Result<()> {
    fs::create_dir_all(fixture_path)?;
    let metadata_path = fixture_path.join("metadata.json");
    fs::write(
        &metadata_path,
        format!(
            r#"{{"name":"{name}","version":"1.0.0","summary":"Regent stub fixture for {name}","dependencies":[]}}"#,
            name = module_name
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
        let module = FixtureModule::new("test").with_repo("https://github.com/test/test.git");

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
        assert_eq!(
            stdlib.repo(),
            Some("https://github.com/puppetlabs/puppetlabs-stdlib.git")
        );
        assert_eq!(stdlib.ref_value(), Some("v9.4.1"));
        let concat = modules.get("concat").unwrap();
        assert_eq!(
            concat.repo(),
            Some("https://github.com/puppetlabs/puppetlabs-concat.git")
        );
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
        manager.config = FixtureConfig::new().add_module("puppet", FixtureModule::new("puppet"));

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
        manager.config = FixtureConfig::new().add_module("puppet", FixtureModule::new("puppet"));

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
        manager.config = FixtureConfig::new().add_module("puppet", FixtureModule::new("puppet"));

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
        manager.config = FixtureConfig::new().add_module("puppet", FixtureModule::new("puppet"));

        assert!(manager.has_fixtures());
    }

    #[test]
    fn test_fixture_manager_parse_empty_fixtures_yml() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let fixtures_yml = temp_dir.path().join("fixtures.yml");
        fs::write(&fixtures_yml, "")?;

        let mut manager = FixtureManager::new(temp_dir.path(), temp_dir.path());
        manager.parse_fixtures_yml(&fixtures_yml)?;

        assert!(
            manager.config.modules.is_none() || manager.config.modules.as_ref().unwrap().is_empty()
        );
        Ok(())
    }

    #[test]
    fn forge_file_uri_picks_current_release_when_unpinned() {
        let metadata = serde_json::json!({
            "current_release": {
                "version": "9.4.1",
                "file_uri": "/v3/files/puppetlabs-stdlib-9.4.1.tar.gz"
            }
        });
        assert_eq!(
            forge_file_uri(&metadata, None),
            Some("/v3/files/puppetlabs-stdlib-9.4.1.tar.gz".to_string())
        );
    }

    #[test]
    fn forge_file_uri_picks_matching_pinned_version() {
        let metadata = serde_json::json!({
            "current_release": {
                "version": "9.4.1",
                "file_uri": "/v3/files/puppetlabs-stdlib-9.4.1.tar.gz"
            },
            "releases": [
                { "version": "9.4.1", "file_uri": "/v3/files/puppetlabs-stdlib-9.4.1.tar.gz" },
                { "version": "9.0.0", "file_uri": "/v3/files/puppetlabs-stdlib-9.0.0.tar.gz" },
                { "version": "8.6.0", "file_uri": "/v3/files/puppetlabs-stdlib-8.6.0.tar.gz" }
            ]
        });
        assert_eq!(
            forge_file_uri(&metadata, Some("9.0.0")),
            Some("/v3/files/puppetlabs-stdlib-9.0.0.tar.gz".to_string())
        );
    }

    #[test]
    fn forge_file_uri_returns_none_for_unknown_pinned_version() {
        let metadata = serde_json::json!({
            "current_release": { "file_uri": "/v3/files/x-y-1.0.0.tar.gz" },
            "releases": [ { "version": "1.0.0", "file_uri": "/v3/files/x-y-1.0.0.tar.gz" } ]
        });
        assert_eq!(forge_file_uri(&metadata, Some("99.0.0")), None);
    }

    #[test]
    fn install_forge_rejects_invalid_slug() {
        let dir = TempDir::new().unwrap();
        let err = install_forge(&dir.path().join("bogus"), "not-a-valid-form-without-slash").err();
        // No slash AND no dash → invalid. Construct one that has neither:
        let err2 = install_forge(&dir.path().join("bogus2"), "noseparators").err();
        assert!(err2.is_some(), "slug without separators should error");
        // The dashed variant is actually valid (author-module), so don't assert on err here.
        let _ = err;
    }

    /// Extracting a tarball laid out like a forge release strips the leading
    /// `<author>-<module>-<version>/` directory.
    #[test]
    fn extract_forge_tarball_strips_top_level_dir() {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use tar::Builder;

        let work = TempDir::new().unwrap();
        let tarball_path = work.path().join("module.tar.gz");

        // Build a tarball whose top dir is `author-mod-1.0.0/` and contains
        // a metadata.json and a manifests/init.pp.
        {
            let file = fs::File::create(&tarball_path).unwrap();
            let gz = GzEncoder::new(file, Compression::default());
            let mut tar = Builder::new(gz);

            let src_dir = work.path().join("author-mod-1.0.0");
            fs::create_dir_all(src_dir.join("manifests")).unwrap();
            fs::write(src_dir.join("metadata.json"), r#"{"name":"author-mod"}"#).unwrap();
            fs::write(src_dir.join("manifests/init.pp"), "class mod {}").unwrap();

            tar.append_dir_all("author-mod-1.0.0", &src_dir).unwrap();
            tar.finish().unwrap();
        }

        let dest = work.path().join("installed");
        extract_forge_tarball(&tarball_path, &dest).unwrap();

        assert!(
            dest.join("metadata.json").exists(),
            "metadata.json must be at the top level"
        );
        assert!(dest.join("manifests").join("init.pp").exists());
        assert!(
            !dest.join("author-mod-1.0.0").exists(),
            "the leading <author-mod-version>/ directory must be stripped"
        );
    }

    /// Live network test — only runs when REGENT_NETWORK_TESTS=1 is set.
    #[test]
    fn install_forge_downloads_real_stdlib() {
        if std::env::var("REGENT_NETWORK_TESTS").ok().as_deref() != Some("1") {
            return;
        }
        let dir = TempDir::new().unwrap();
        let fixture = dir.path().join("stdlib");
        install_forge(&fixture, "puppetlabs/stdlib").expect("forge download succeeds");
        assert!(fixture.join("metadata.json").exists());
        assert!(fixture.join("manifests").exists());
    }

    #[test]
    fn test_fixture_manager_metadata_creation() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let fixtures_dir = temp_dir.path().join("fixtures");

        let mut manager = FixtureManager::new(temp_dir.path(), &fixtures_dir);
        manager.config = FixtureConfig::new().add_module("test", FixtureModule::new("test"));

        manager.setup_fixtures()?;

        let metadata_path = fixtures_dir.join("test").join("metadata.json");
        assert!(metadata_path.exists());

        let content = fs::read_to_string(&metadata_path)?;
        assert!(content.contains("test"));
        assert!(content.contains("1.0.0"));

        Ok(())
    }

    #[test]
    fn cache_key_is_stable_and_sanitized() {
        assert_eq!(
            cache_key(&FixtureSource::Forge {
                slug: "puppetlabs/stdlib:9.4.1".into()
            }),
            "forge_puppetlabs-stdlib-9.4.1"
        );
        // Unpinned forge gets a `-current` suffix.
        assert_eq!(
            cache_key(&FixtureSource::Forge {
                slug: "voxpupuli/archive".into()
            }),
            "forge_voxpupuli-archive-current"
        );
        // Git keys fold the URL/ref into one safe segment.
        let git = cache_key(&FixtureSource::Git {
            repo: "https://github.com/puppetlabs/puppetlabs-concat.git".into(),
            ref_value: Some("v9.0.0".into()),
        });
        assert!(git.starts_with("git_"));
        assert!(!git.contains('/'));
        assert!(git.contains("v9.0.0"));
    }

    /// Build a minimal valid cached module directory.
    fn write_fake_cached_module(dir: &Path, name: &str) {
        fs::create_dir_all(dir.join("manifests")).unwrap();
        fs::write(
            dir.join("metadata.json"),
            format!(r#"{{"name":"{name}","version":"2.0.0","dependencies":[]}}"#),
        )
        .unwrap();
        fs::write(dir.join("manifests").join("init.pp"), "class archive {}\n").unwrap();
    }

    #[test]
    fn offline_install_uses_cache_without_network() -> Result<()> {
        let temp = TempDir::new()?;
        let cache = temp.path().join("cache");
        let fixtures_dir = temp.path().join("fixtures");

        // Pre-populate the cache as if a prior online run had downloaded it.
        let source = FixtureSource::Forge {
            slug: "voxpupuli/archive".into(),
        };
        let cached = cache.join(cache_key(&source));
        write_fake_cached_module(&cached, "archive");

        let mut manager = FixtureManager::new(temp.path(), &fixtures_dir);
        manager.set_cache_dir(Some(cache.clone())).set_offline(true);
        manager.config = FixtureConfig::new().add_module(
            "archive",
            FixtureModule::new("archive").with_forge("voxpupuli/archive"),
        );

        let count = manager.setup_fixtures()?;
        assert_eq!(count, 1);
        // The module was copied out of the cache — contents and all.
        assert!(fixtures_dir.join("archive").join("metadata.json").exists());
        assert!(fixtures_dir
            .join("archive")
            .join("manifests")
            .join("init.pp")
            .exists());
        let meta = fs::read_to_string(fixtures_dir.join("archive").join("metadata.json"))?;
        assert!(
            meta.contains("2.0.0"),
            "should be the cached module, not a stub"
        );
        Ok(())
    }

    #[test]
    fn offline_install_falls_back_to_stub_on_cache_miss() -> Result<()> {
        let temp = TempDir::new()?;
        let cache = temp.path().join("empty-cache");
        let fixtures_dir = temp.path().join("fixtures");

        let mut manager = FixtureManager::new(temp.path(), &fixtures_dir);
        manager.set_cache_dir(Some(cache)).set_offline(true);
        manager.config = FixtureConfig::new().add_module(
            "archive",
            FixtureModule::new("archive").with_forge("voxpupuli/archive"),
        );

        let count = manager.setup_fixtures()?;
        assert_eq!(count, 1);
        // No network in offline mode → a stub is written.
        let meta = fs::read_to_string(fixtures_dir.join("archive").join("metadata.json"))?;
        assert!(
            meta.contains("Regent stub fixture"),
            "offline miss should stub, got: {meta}"
        );
        Ok(())
    }
}
