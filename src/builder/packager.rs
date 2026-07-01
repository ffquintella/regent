use anyhow::{Context, Result};
use bzip2::write::BzEncoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs::{self, File};
use std::io::{copy, Write};
use std::path::{Path, PathBuf};
use tar::Builder as TarBuilder;
use walkdir::WalkDir;
use zip::write::FileOptions;
use zip::CompressionMethod;
use zip::ZipWriter;

/// Output format for module package
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum BuildFormat {
    /// tar.gz (gzip compression) - default Puppet format
    #[default]
    TarGz,
    /// tar.bz2 (bzip2 compression) - better compression
    TarBz2,
    /// ZIP format - Windows compatibility
    Zip,
}


/// Configuration for building a module package
#[derive(Debug, Clone)]
pub struct PackagerConfig {
    /// Path to the module directory
    pub module_path: PathBuf,
    /// Output directory for the built package (defaults to pkg/)
    pub output_dir: Option<PathBuf>,
    /// Custom version override (if not specified, uses metadata.json version)
    pub version_override: Option<String>,
    /// Whether to include .pdkignore filtering
    pub respect_ignore_files: bool,
    /// Output format for the package
    pub format: BuildFormat,
}

impl PackagerConfig {
    pub fn new(module_path: impl Into<PathBuf>) -> Self {
        Self {
            module_path: module_path.into(),
            output_dir: None,
            version_override: None,
            respect_ignore_files: true,
            format: BuildFormat::default(),
        }
    }

    pub fn with_output_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.output_dir = Some(dir.into());
        self
    }

    pub fn with_version(mut self, version: String) -> Self {
        self.version_override = Some(version);
        self
    }

    pub fn ignore_files(mut self, respect: bool) -> Self {
        self.respect_ignore_files = respect;
        self
    }

    pub fn with_format(mut self, format: BuildFormat) -> Self {
        self.format = format;
        self
    }
}

/// Builds tarball packages for Puppet modules
pub struct TarballBuilder {
    config: PackagerConfig,
    ignore_patterns: Vec<String>,
}

impl TarballBuilder {
    /// Create a new TarballBuilder with the given configuration
    pub fn new(config: PackagerConfig) -> Result<Self> {
        let mut builder = Self {
            config,
            ignore_patterns: Vec::new(),
        };

        if builder.config.respect_ignore_files {
            builder.load_ignore_patterns()?;
        }

        Ok(builder)
    }

    /// Load ignore patterns from .pdkignore and .gitignore files
    fn load_ignore_patterns(&mut self) -> Result<()> {
        // Always ignore common build artifacts and temporary files
        self.ignore_patterns.extend(vec![
            ".git/".to_string(),
            ".git".to_string(),
            "pkg/".to_string(),
            "*.swp".to_string(),
            "*.swo".to_string(),
            "*~".to_string(),
            ".DS_Store".to_string(),
        ]);

        // Load .pdkignore
        let pdkignore_path = self.config.module_path.join(".pdkignore");
        if pdkignore_path.exists() {
            let content =
                fs::read_to_string(&pdkignore_path).context("Failed to read .pdkignore")?;
            for line in content.lines() {
                let line = line.trim();
                if !line.is_empty() && !line.starts_with('#') {
                    self.ignore_patterns.push(line.to_string());
                }
            }
        }

        // Load .gitignore (lower priority than .pdkignore)
        let gitignore_path = self.config.module_path.join(".gitignore");
        if gitignore_path.exists() {
            let content =
                fs::read_to_string(&gitignore_path).context("Failed to read .gitignore")?;
            for line in content.lines() {
                let line = line.trim();
                if !line.is_empty() && !line.starts_with('#') {
                    // Avoid duplicates
                    if !self.ignore_patterns.contains(&line.to_string()) {
                        self.ignore_patterns.push(line.to_string());
                    }
                }
            }
        }

        Ok(())
    }

    /// Check if a path should be ignored based on ignore patterns.
    ///
    /// Patterns follow gitignore-style semantics:
    /// - A leading `/` anchors the pattern to the module root (relative path start).
    /// - A trailing `/` matches directories only (but here we also match anything under it).
    /// - `*` matches any sequence of characters within a path segment.
    /// - Without a leading `/`, the pattern matches at any depth.
    fn should_ignore(&self, path: &Path) -> bool {
        // Normalize to forward slashes so patterns work on Windows too.
        let path_str = path.to_string_lossy().replace('\\', "/");
        let path_str = path_str.trim_start_matches("./");

        for pattern in &self.ignore_patterns {
            let (anchored, pat) = if let Some(rest) = pattern.strip_prefix('/') {
                (true, rest)
            } else {
                (false, pattern.as_str())
            };

            let (is_dir_pattern, pat) = if let Some(rest) = pat.strip_suffix('/') {
                (true, rest)
            } else {
                (false, pat)
            };

            if pat.is_empty() {
                continue;
            }

            if pat.contains('*') || pat.contains('?') {
                // Translate glob to regex. `*` matches anything except `/` (single segment);
                // `**` matches across segments.
                let mut regex_src = String::new();
                regex_src.push('^');
                if !anchored {
                    // Allow matching at any depth.
                    regex_src.push_str("(?:.*/)?");
                }
                let mut chars = pat.chars().peekable();
                while let Some(c) = chars.next() {
                    match c {
                        '*' => {
                            if chars.peek() == Some(&'*') {
                                chars.next();
                                regex_src.push_str(".*");
                            } else {
                                regex_src.push_str("[^/]*");
                            }
                        }
                        '?' => regex_src.push_str("[^/]"),
                        '.' | '+' | '(' | ')' | '|' | '[' | ']' | '{' | '}' | '^' | '$' | '\\' => {
                            regex_src.push('\\');
                            regex_src.push(c);
                        }
                        _ => regex_src.push(c),
                    }
                }
                // A trailing-slash (dir) pattern and a plain pattern match the
                // same way here: the path itself or anything under it (see the
                // `is_dir_pattern` note below).
                regex_src.push_str("(?:/.*)?$");
                if let Ok(re) = regex::Regex::new(&regex_src) {
                    if re.is_match(path_str) {
                        return true;
                    }
                }
            } else if anchored {
                // Anchored literal: must match at the start of the relative path,
                // either exactly or as a path prefix.
                if path_str == pat || path_str.starts_with(&format!("{}/", pat)) {
                    return true;
                }
            } else {
                // Unanchored literal: match any path segment equal to `pat`,
                // or any subtree rooted at such a segment.
                let matches_segment = path_str == pat
                    || path_str.starts_with(&format!("{}/", pat))
                    || path_str.ends_with(&format!("/{}", pat))
                    || path_str.contains(&format!("/{}/", pat));
                if matches_segment {
                    return true;
                }
            }
            let _ = is_dir_pattern; // semantics: we match files under a dir pattern too
        }

        false
    }

    /// Build the package in the specified format
    pub fn build(&self, module_name: &str, version: &str) -> Result<PathBuf> {
        match self.config.format {
            BuildFormat::TarGz => self.build_tar_gz(module_name, version),
            BuildFormat::TarBz2 => self.build_tar_bz2(module_name, version),
            BuildFormat::Zip => self.build_zip(module_name, version),
        }
    }

    /// Build tar.gz package
    fn build_tar_gz(&self, module_name: &str, version: &str) -> Result<PathBuf> {
        let output_dir = self.get_output_dir();
        fs::create_dir_all(&output_dir).context("Failed to create output directory")?;

        let filename = format!("{}-{}.tar.gz", module_name, version);
        let package_path = output_dir.join(&filename);

        let tar_file = File::create(&package_path).context("Failed to create tarball file")?;
        let encoder = GzEncoder::new(tar_file, Compression::default());
        let mut tar = TarBuilder::new(encoder);

        self.add_directory_to_tar(&mut tar, &self.config.module_path, module_name, version)?;

        tar.into_inner()
            .context("Failed to finalize gzip encoding")?
            .finish()
            .context("Failed to finalize tarball")?;

        Ok(package_path)
    }

    /// Build tar.bz2 package
    fn build_tar_bz2(&self, module_name: &str, version: &str) -> Result<PathBuf> {
        let output_dir = self.get_output_dir();
        fs::create_dir_all(&output_dir).context("Failed to create output directory")?;

        let filename = format!("{}-{}.tar.bz2", module_name, version);
        let package_path = output_dir.join(&filename);

        let tar_file = File::create(&package_path).context("Failed to create tarball file")?;
        let encoder = BzEncoder::new(tar_file, bzip2::Compression::default());
        let mut tar = TarBuilder::new(encoder);

        self.add_directory_to_tar(&mut tar, &self.config.module_path, module_name, version)?;

        tar.into_inner()
            .context("Failed to finalize bzip2 encoding")?
            .finish()
            .context("Failed to finalize tarball")?;

        Ok(package_path)
    }

    /// Build ZIP package
    fn build_zip(&self, module_name: &str, version: &str) -> Result<PathBuf> {
        let output_dir = self.get_output_dir();
        fs::create_dir_all(&output_dir).context("Failed to create output directory")?;

        let filename = format!("{}-{}.zip", module_name, version);
        let package_path = output_dir.join(&filename);

        let zip_file = File::create(&package_path).context("Failed to create ZIP file")?;
        let mut zip = ZipWriter::new(zip_file);
        let options = FileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(0o755);

        self.add_directory_to_zip(
            &mut zip,
            &self.config.module_path,
            module_name,
            version,
            options,
        )?;

        zip.finish().context("Failed to finalize ZIP file")?;

        Ok(package_path)
    }

    /// Get output directory
    fn get_output_dir(&self) -> PathBuf {
        self.config
            .output_dir.clone()
            .unwrap_or_else(|| self.config.module_path.join("pkg"))
    }

    /// Recursively add directory contents to the tar archive
    fn add_directory_to_tar<W: Write>(
        &self,
        tar: &mut TarBuilder<W>,
        base_path: &Path,
        module_name: &str,
        version: &str,
    ) -> Result<()> {
        let prefix = format!("{}-{}", module_name, version);

        for entry in WalkDir::new(base_path)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| !self.should_ignore_entry(e, base_path))
        {
            let entry = entry.context("Failed to read directory entry")?;
            let path = entry.path();

            // Skip the base directory itself
            if path == base_path {
                continue;
            }

            // Calculate relative path
            let rel_path = path
                .strip_prefix(base_path)
                .context("Failed to calculate relative path")?;

            // Skip if ignored
            if self.should_ignore(rel_path) {
                continue;
            }

            // Create tar entry path: module-name-version/relative/path
            let tar_path = PathBuf::from(&prefix).join(rel_path);

            if path.is_file() {
                // Add file to tarball
                let mut file =
                    File::open(path).with_context(|| format!("Failed to open file: {:?}", path))?;

                tar.append_file(&tar_path, &mut file)
                    .with_context(|| format!("Failed to add file to tarball: {:?}", tar_path))?;
            } else if path.is_dir() {
                // Add directory to tarball
                tar.append_dir(&tar_path, path).with_context(|| {
                    format!("Failed to add directory to tarball: {:?}", tar_path)
                })?;
            }
        }

        Ok(())
    }

    /// Helper to check if a WalkDir entry should be ignored
    fn should_ignore_entry(&self, entry: &walkdir::DirEntry, base_path: &Path) -> bool {
        if let Ok(rel_path) = entry.path().strip_prefix(base_path) {
            self.should_ignore(rel_path)
        } else {
            false
        }
    }

    /// Recursively add directory contents to ZIP archive
    fn add_directory_to_zip<W: Write + std::io::Seek>(
        &self,
        zip: &mut ZipWriter<W>,
        base_path: &Path,
        module_name: &str,
        version: &str,
        options: FileOptions<()>,
    ) -> Result<()> {
        let prefix = format!("{}-{}", module_name, version);

        for entry in WalkDir::new(base_path)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| !self.should_ignore_entry(e, base_path))
        {
            let entry = entry.context("Failed to read directory entry")?;
            let path = entry.path();

            // Skip the base directory itself
            if path == base_path {
                continue;
            }

            // Calculate relative path
            let rel_path = path
                .strip_prefix(base_path)
                .context("Failed to calculate relative path")?;

            // Skip if ignored
            if self.should_ignore(rel_path) {
                continue;
            }

            // Create ZIP entry path: module-name-version/relative/path
            let zip_path = PathBuf::from(&prefix).join(rel_path);
            let zip_path_str = zip_path.to_string_lossy().replace("\\", "/");

            if path.is_file() {
                // Add file to ZIP
                zip.start_file(&zip_path_str, options)
                    .with_context(|| format!("Failed to start ZIP file entry: {:?}", zip_path))?;

                let mut file =
                    File::open(path).with_context(|| format!("Failed to open file: {:?}", path))?;

                copy(&mut file, zip)
                    .with_context(|| format!("Failed to write file to ZIP: {:?}", zip_path))?;
            } else if path.is_dir() {
                // Add directory to ZIP (with trailing slash)
                let dir_path = format!("{}/", zip_path_str);
                zip.add_directory(&dir_path, options)
                    .with_context(|| format!("Failed to add directory to ZIP: {:?}", zip_path))?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::read::GzDecoder;
    use tar::Archive;
    use tempfile::TempDir;

    fn create_test_module(dir: &Path) -> Result<()> {
        // Create metadata.json
        fs::write(
            dir.join("metadata.json"),
            r#"{
                "name": "testuser-testmodule",
                "version": "1.0.0",
                "author": "testuser",
                "license": "Apache-2.0",
                "summary": "Test module",
                "source": "https://github.com/test/test",
                "dependencies": []
            }"#,
        )?;

        // Create manifests directory
        fs::create_dir(dir.join("manifests"))?;
        fs::write(
            dir.join("manifests").join("init.pp"),
            "class testmodule { }\n",
        )?;

        // Create files directory
        fs::create_dir(dir.join("files"))?;
        fs::write(dir.join("files").join("example.txt"), "example content\n")?;

        // Create README
        fs::write(dir.join("README.md"), "# Test Module\n")?;

        Ok(())
    }

    #[test]
    fn test_packager_config_builder() {
        let config = PackagerConfig::new("/tmp/module")
            .with_output_dir("/tmp/output")
            .with_version("2.0.0".to_string())
            .ignore_files(false);

        assert_eq!(config.module_path, PathBuf::from("/tmp/module"));
        assert_eq!(config.output_dir, Some(PathBuf::from("/tmp/output")));
        assert_eq!(config.version_override, Some("2.0.0".to_string()));
        assert!(!config.respect_ignore_files);
    }

    #[test]
    fn test_tarball_builder_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = PackagerConfig::new(temp_dir.path());
        let builder = TarballBuilder::new(config);

        assert!(builder.is_ok());
    }

    #[test]
    fn test_ignore_patterns_loaded() {
        let temp_dir = TempDir::new().unwrap();

        // Create .pdkignore
        fs::write(
            temp_dir.path().join(".pdkignore"),
            "*.tmp\n# Comment\ntest/\n",
        )
        .unwrap();

        let config = PackagerConfig::new(temp_dir.path());
        let builder = TarballBuilder::new(config).unwrap();

        // Should have default patterns + pdkignore patterns
        assert!(builder.ignore_patterns.len() > 3);
        assert!(builder.ignore_patterns.contains(&"*.tmp".to_string()));
        assert!(builder.ignore_patterns.contains(&"test/".to_string()));
    }

    #[test]
    fn test_should_ignore_patterns() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join(".pdkignore"), "*.swp\ntemp/\n").unwrap();

        let config = PackagerConfig::new(temp_dir.path());
        let builder = TarballBuilder::new(config).unwrap();

        assert!(builder.should_ignore(Path::new("file.swp")));
        assert!(builder.should_ignore(Path::new("temp/file.txt")));
        assert!(builder.should_ignore(Path::new(".git/config")));
        assert!(builder.should_ignore(Path::new("pkg/module.tar.gz")));
        assert!(!builder.should_ignore(Path::new("manifests/init.pp")));
    }

    #[test]
    fn test_should_ignore_leading_slash_anchored() {
        // Regression: anchored gitignore-style patterns like `/vendor/` and `/bin/`
        // must actually filter the corresponding top-level directories.
        let temp_dir = TempDir::new().unwrap();
        fs::write(
            temp_dir.path().join(".pdkignore"),
            "/vendor/\n/bin/\n/spec/fixtures/modules/*\n/pkg/\n",
        )
        .unwrap();

        let config = PackagerConfig::new(temp_dir.path());
        let builder = TarballBuilder::new(config).unwrap();

        assert!(builder.should_ignore(Path::new("vendor/bundle/ruby/foo.rb")));
        assert!(builder.should_ignore(Path::new("bin/something")));
        assert!(builder.should_ignore(Path::new("pkg/module.tar.gz")));
        assert!(builder.should_ignore(Path::new("spec/fixtures/modules/stdlib")));
        // Anchored: must not match nested occurrences.
        assert!(!builder.should_ignore(Path::new("manifests/bin/file.pp")));
        assert!(!builder.should_ignore(Path::new("files/vendor/x.txt")));
        // Real module content stays in.
        assert!(!builder.should_ignore(Path::new("manifests/init.pp")));
        assert!(!builder.should_ignore(Path::new("metadata.json")));
    }

    #[test]
    fn test_build_creates_tarball() {
        let temp_dir = TempDir::new().unwrap();
        create_test_module(temp_dir.path()).unwrap();

        let config =
            PackagerConfig::new(temp_dir.path()).with_output_dir(temp_dir.path().join("output"));
        let builder = TarballBuilder::new(config).unwrap();

        let tarball_path = builder.build("testuser-testmodule", "1.0.0").unwrap();

        assert!(tarball_path.exists());
        assert!(tarball_path.ends_with("testuser-testmodule-1.0.0.tar.gz"));
    }

    #[test]
    fn test_tarball_includes_manifests() {
        let temp_dir = TempDir::new().unwrap();
        create_test_module(temp_dir.path()).unwrap();

        let config = PackagerConfig::new(temp_dir.path());
        let builder = TarballBuilder::new(config).unwrap();

        let tarball_path = builder.build("testuser-testmodule", "1.0.0").unwrap();

        // Extract and verify contents
        let tar_file = File::open(&tarball_path).unwrap();
        let decoder = GzDecoder::new(tar_file);
        let mut archive = Archive::new(decoder);

        let entries: Vec<String> = archive
            .entries()
            .unwrap()
            .filter_map(|e| e.ok())
            .filter_map(|e| e.path().ok().map(|p| p.to_string_lossy().to_string()))
            .collect();

        assert!(entries.iter().any(|p| p.contains("manifests/init.pp")));
        assert!(entries.iter().any(|p| p.contains("metadata.json")));
        assert!(entries.iter().any(|p| p.contains("README.md")));
    }

    #[test]
    fn test_tarball_excludes_git() {
        let temp_dir = TempDir::new().unwrap();
        create_test_module(temp_dir.path()).unwrap();

        // Create .git directory
        fs::create_dir(temp_dir.path().join(".git")).unwrap();
        fs::write(temp_dir.path().join(".git").join("config"), "git config\n").unwrap();

        let config = PackagerConfig::new(temp_dir.path());
        let builder = TarballBuilder::new(config).unwrap();

        let tarball_path = builder.build("testuser-testmodule", "1.0.0").unwrap();

        // Extract and verify .git is not included
        let tar_file = File::open(&tarball_path).unwrap();
        let decoder = GzDecoder::new(tar_file);
        let mut archive = Archive::new(decoder);

        let entries: Vec<String> = archive
            .entries()
            .unwrap()
            .filter_map(|e| e.ok())
            .filter_map(|e| e.path().ok().map(|p| p.to_string_lossy().to_string()))
            .collect();

        assert!(!entries.iter().any(|p| p.contains(".git")));
    }

    #[test]
    fn test_tarball_respects_pdkignore() {
        let temp_dir = TempDir::new().unwrap();
        create_test_module(temp_dir.path()).unwrap();

        // Create .pdkignore
        fs::write(temp_dir.path().join(".pdkignore"), "files/\n").unwrap();

        let config = PackagerConfig::new(temp_dir.path());
        let builder = TarballBuilder::new(config).unwrap();

        let tarball_path = builder.build("testuser-testmodule", "1.0.0").unwrap();

        // Extract and verify files/ is not included
        let tar_file = File::open(&tarball_path).unwrap();
        let decoder = GzDecoder::new(tar_file);
        let mut archive = Archive::new(decoder);

        let entries: Vec<String> = archive
            .entries()
            .unwrap()
            .filter_map(|e| e.ok())
            .filter_map(|e| e.path().ok().map(|p| p.to_string_lossy().to_string()))
            .collect();

        assert!(!entries.iter().any(|p| p.contains("files/")));
        assert!(entries.iter().any(|p| p.contains("manifests/init.pp")));
    }

    #[test]
    fn test_custom_output_directory() {
        let temp_dir = TempDir::new().unwrap();
        create_test_module(temp_dir.path()).unwrap();

        let custom_output = temp_dir.path().join("custom_output");
        let config = PackagerConfig::new(temp_dir.path()).with_output_dir(&custom_output);
        let builder = TarballBuilder::new(config).unwrap();

        let tarball_path = builder.build("testuser-testmodule", "1.0.0").unwrap();

        assert!(tarball_path.starts_with(&custom_output));
        assert!(tarball_path.exists());
    }

    #[test]
    fn test_build_format_tar_bz2() {
        let temp_dir = TempDir::new().unwrap();
        create_test_module(temp_dir.path()).unwrap();

        let config = PackagerConfig::new(temp_dir.path()).with_format(BuildFormat::TarBz2);
        let builder = TarballBuilder::new(config).unwrap();

        let package_path = builder.build("testuser-testmodule", "1.0.0").unwrap();

        assert!(package_path.exists());
        assert!(package_path.to_string_lossy().ends_with(".tar.bz2"));
    }

    #[test]
    fn test_build_format_zip() {
        let temp_dir = TempDir::new().unwrap();
        create_test_module(temp_dir.path()).unwrap();

        let config = PackagerConfig::new(temp_dir.path()).with_format(BuildFormat::Zip);
        let builder = TarballBuilder::new(config).unwrap();

        let package_path = builder.build("testuser-testmodule", "1.0.0").unwrap();

        assert!(package_path.exists());
        assert!(package_path.to_string_lossy().ends_with(".zip"));
    }

    #[test]
    fn test_zip_includes_manifests() {
        let temp_dir = TempDir::new().unwrap();
        create_test_module(temp_dir.path()).unwrap();

        let config = PackagerConfig::new(temp_dir.path()).with_format(BuildFormat::Zip);
        let builder = TarballBuilder::new(config).unwrap();

        let package_path = builder.build("testuser-testmodule", "1.0.0").unwrap();

        // Verify ZIP contents
        let zip_file = File::open(&package_path).unwrap();
        let mut archive = zip::ZipArchive::new(zip_file).unwrap();

        let mut file_names = Vec::new();
        for i in 0..archive.len() {
            if let Ok(file) = archive.by_index(i) {
                file_names.push(file.name().to_string());
            }
        }

        assert!(file_names.iter().any(|p| p.contains("manifests/init.pp")));
        assert!(file_names.iter().any(|p| p.contains("metadata.json")));
    }
}
