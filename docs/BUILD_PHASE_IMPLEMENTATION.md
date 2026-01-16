# 🏗️ Build Phase Implementation Guide - Detailed

## Phase 1: BUILD FUNCTIONALITY - Complete Implementation Plan

This document provides step-by-step implementation details for Phase 1 (Build Functionality).

---

## 📋 Overview

### Goals
1. Create production-ready tarball packages
2. Manage module metadata completely
3. Resolve dependencies
4. Support multiple output formats
5. Achieve feature parity with PDK build

### Timeline
- Week 1: Metadata management
- Week 2: Core packaging
- Week 3: Advanced features & polish

### Success Metrics
- ✅ Build packages compatible with Puppet Forge
- ✅ Checksums generated correctly
- ✅ All 40+ tests passing
- ✅ Build time < 500ms
- ✅ Documentation complete

---

## Week 1: Metadata Management

### Task 1.1.1: Create MetadataManager

**File**: `src/builder/metadata.rs`

```rust
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use anyhow::{anyhow, Result};
use semver::Version;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleDependency {
    pub name: String,
    pub version_requirement: String,  // e.g., ">= 1.0.0"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Requirement {
    pub name: String,
    pub version_requirement: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OSSupport {
    pub operatingsystem: String,
    #[serde(default)]
    pub operatingsystemrelease: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleMetadata {
    pub name: String,
    pub version: String,
    pub author: String,
    pub license: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub project_page: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub issues_url: String,
    #[serde(default)]
    pub dependencies: Vec<ModuleDependency>,
    #[serde(default)]
    pub requirements: Vec<Requirement>,
    #[serde(default)]
    pub operatingsystem_support: Vec<OSSupport>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub template_version: String,
}

impl ModuleMetadata {
    /// Load metadata from metadata.json file
    pub fn load(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let metadata: ModuleMetadata = serde_json::from_str(&content)?;
        metadata.validate()?;
        Ok(metadata)
    }

    /// Validate metadata fields
    pub fn validate(&self) -> Result<()> {
        // Validate name format
        if !self.name.contains('-') && !self.name.contains('::') {
            return Err(anyhow!("Module name must contain '-' or '::'"));
        }

        // Validate version format (semver)
        Version::parse(&self.version)
            .map_err(|e| anyhow!("Invalid version format: {}", e))?;

        // Validate author
        if self.author.is_empty() {
            return Err(anyhow!("Author field is required"));
        }

        // Validate license
        if self.license.is_empty() {
            return Err(anyhow!("License field is required"));
        }

        // Validate dependencies
        for dep in &self.dependencies {
            self.validate_version_requirement(&dep.version_requirement)?;
        }

        // Validate requirements
        for req in &self.requirements {
            self.validate_version_requirement(&req.version_requirement)?;
        }

        Ok(())
    }

    /// Validate version requirement string
    fn validate_version_requirement(&self, req: &str) -> Result<()> {
        // Simple validation for patterns like ">=1.0.0", "~>2.0", etc.
        let patterns = vec![">=", "<=", ">", "<", "~>", "="];
        let has_valid_pattern = patterns.iter().any(|p| req.starts_with(p));

        if !has_valid_pattern {
            return Err(anyhow!("Invalid version requirement: {}", req));
        }

        Ok(())
    }

    /// Get module filename (without .tar.gz)
    pub fn get_module_filename(&self) -> String {
        format!("{}-{}", self.name.replace("::", "-"), self.version)
    }

    /// Increment patch version
    pub fn bump_patch(&mut self) -> Result<()> {
        let mut version = Version::parse(&self.version)?;
        version.patch += 1;
        self.version = version.to_string();
        Ok(())
    }

    /// Increment minor version
    pub fn bump_minor(&mut self) -> Result<()> {
        let mut version = Version::parse(&self.version)?;
        version.minor += 1;
        version.patch = 0;
        self.version = version.to_string();
        Ok(())
    }

    /// Increment major version
    pub fn bump_major(&mut self) -> Result<()> {
        let mut version = Version::parse(&self.version)?;
        version.major += 1;
        version.minor = 0;
        version.patch = 0;
        self.version = version.to_string();
        Ok(())
    }

    /// Save metadata to file
    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)?;
        Ok(())
    }

    /// Check if Puppet requirement exists
    pub fn get_puppet_requirement(&self) -> Option<&str> {
        self.requirements
            .iter()
            .find(|r| r.name == "puppet")
            .map(|r| r.version_requirement.as_str())
    }

    /// Get module short name
    pub fn get_short_name(&self) -> &str {
        self.name.split('-').last().unwrap_or(&self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_metadata() -> ModuleMetadata {
        ModuleMetadata {
            name: "user-mymodule".to_string(),
            version: "1.0.0".to_string(),
            author: "Test Author".to_string(),
            license: "Apache-2.0".to_string(),
            summary: "Test module".to_string(),
            description: String::new(),
            project_page: String::new(),
            source: String::new(),
            issues_url: String::new(),
            dependencies: vec![],
            requirements: vec![Requirement {
                name: "puppet".to_string(),
                version_requirement: ">= 6.0.0".to_string(),
            }],
            operatingsystem_support: vec![],
            tags: vec![],
            template_version: String::new(),
        }
    }

    #[test]
    fn test_metadata_validation_valid() {
        let metadata = create_test_metadata();
        assert!(metadata.validate().is_ok());
    }

    #[test]
    fn test_metadata_validation_invalid_name() {
        let mut metadata = create_test_metadata();
        metadata.name = "invalid".to_string();
        assert!(metadata.validate().is_err());
    }

    #[test]
    fn test_metadata_validation_invalid_version() {
        let mut metadata = create_test_metadata();
        metadata.version = "not-a-version".to_string();
        assert!(metadata.validate().is_err());
    }

    #[test]
    fn test_bump_patch_version() {
        let mut metadata = create_test_metadata();
        metadata.bump_patch().unwrap();
        assert_eq!(metadata.version, "1.0.1");
    }

    #[test]
    fn test_bump_minor_version() {
        let mut metadata = create_test_metadata();
        metadata.bump_minor().unwrap();
        assert_eq!(metadata.version, "1.1.0");
    }

    #[test]
    fn test_bump_major_version() {
        let mut metadata = create_test_metadata();
        metadata.bump_major().unwrap();
        assert_eq!(metadata.version, "2.0.0");
    }

    #[test]
    fn test_get_module_filename() {
        let metadata = create_test_metadata();
        assert_eq!(
            metadata.get_module_filename(),
            "user-mymodule-1.0.0"
        );
    }

    #[test]
    fn test_get_puppet_requirement() {
        let metadata = create_test_metadata();
        let req = metadata.get_puppet_requirement().unwrap();
        assert_eq!(req, ">= 6.0.0");
    }
}
```

**Tests Required**: ✅ 8 tests provided

**Checklist**:
- [ ] Implement ModuleMetadata struct
- [ ] Add serde for JSON serialization
- [ ] Implement validation logic
- [ ] Add version bumping
- [ ] Write all 8 tests
- [ ] Test with real metadata.json

---

### Task 1.1.2: Create Checksum Generator

**File**: `src/builder/checksum.rs`

```rust
use sha2::{Sha256, Digest};
use md5;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use anyhow::Result;

pub struct ChecksumGenerator {
    pub file_path: PathBuf,
}

impl ChecksumGenerator {
    pub fn new(file_path: &Path) -> Self {
        Self {
            file_path: file_path.to_path_buf(),
        }
    }

    /// Generate SHA256 checksum
    pub fn sha256(&self) -> Result<String> {
        let file = File::open(&self.file_path)?;
        let mut reader = BufReader::new(file);
        let mut hasher = Sha256::new();
        let mut buffer = [0; 8192];

        loop {
            let count = reader.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            hasher.update(&buffer[..count]);
        }

        Ok(format!("{:x}", hasher.finalize()))
    }

    /// Generate MD5 checksum
    pub fn md5(&self) -> Result<String> {
        let file = File::open(&self.file_path)?;
        let mut reader = BufReader::new(file);
        let mut context = md5::Context::new();
        let mut buffer = [0; 8192];

        loop {
            let count = reader.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            context.consume(&buffer[..count]);
        }

        Ok(format!("{:x}", context.compute()))
    }

    /// Generate both checksums
    pub fn generate_all(&self) -> Result<ChecksumSet> {
        Ok(ChecksumSet {
            sha256: self.sha256()?,
            md5: self.md5()?,
        })
    }
}

pub struct ChecksumSet {
    pub sha256: String,
    pub md5: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_generation() {
        // Create temp file and test
    }

    #[test]
    fn test_md5_generation() {
        // Create temp file and test
    }
}
```

**Checklist**:
- [ ] Implement SHA256 hashing
- [ ] Implement MD5 hashing
- [ ] Buffer reading for large files
- [ ] Write tests (3+ tests)

---

## Week 2: Core Packaging

### Task 2.1: Implement TarballBuilder

**File**: `src/builder/packager.rs`

```rust
use tar::Builder;
use flate2::Compression;
use flate2::write::GzEncoder;
use std::fs::{File, metadata};
use std::path::{Path, PathBuf};
use anyhow::Result;
use crate::builder::metadata::ModuleMetadata;

pub struct TarballBuilder {
    module_path: PathBuf,
    metadata: ModuleMetadata,
    exclude_patterns: Vec<String>,
}

impl TarballBuilder {
    pub fn new(module_path: &Path, metadata: ModuleMetadata) -> Self {
        Self {
            module_path: module_path.to_path_buf(),
            metadata,
            exclude_patterns: default_exclude_patterns(),
        }
    }

    /// Set custom exclude patterns
    pub fn with_excludes(mut self, patterns: Vec<String>) -> Self {
        self.exclude_patterns = patterns;
        self
    }

    /// Build the tarball
    pub fn build(&self, output_path: &Path) -> Result<BuildInfo> {
        let tar_file = File::create(output_path)?;
        let gz = GzEncoder::new(tar_file, Compression::default());
        let mut tar = Builder::new(gz);

        // Add files from module directory
        self.add_files_to_tar(&mut tar)?;

        // Generate metadata file
        let metadata_json = serde_json::to_string_pretty(&self.metadata)?;
        let mut header = tar::Header::new_old();
        header.set_path("METADATA.json")?;
        header.set_size(metadata_json.len() as u64);
        header.set_mtime(std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs());
        header.set_cksum();
        
        tar.append(&header, metadata_json.as_bytes())?;

        tar.finish()?;

        Ok(BuildInfo {
            filename: self.metadata.get_module_filename(),
            size: metadata(output_path)?.len(),
        })
    }

    fn add_files_to_tar(&self, tar: &mut Builder<GzEncoder<File>>) -> Result<()> {
        for entry in walkdir::WalkDir::new(&self.module_path) {
            let entry = entry?;
            let path = entry.path();
            let relative_path = path.strip_prefix(&self.module_path)?;

            // Check if should exclude
            if self.should_exclude(relative_path) {
                continue;
            }

            if path.is_file() {
                tar.append_path_with_parent(path, &self.metadata.name)?;
            }
        }
        Ok(())
    }

    fn should_exclude(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        
        // Default exclusions
        let default_excludes = vec![
            "pkg/", ".git", ".gitignore", "Gemfile.lock",
            ".rspec", ".fixtures.yml.local", "spec/fixtures/",
            "coverage/", ".coverage",
        ];

        for exclude in default_excludes {
            if path_str.contains(exclude) {
                return true;
            }
        }

        // Custom exclusions
        for pattern in &self.exclude_patterns {
            if path_str.contains(pattern) {
                return true;
            }
        }

        false
    }
}

pub struct BuildInfo {
    pub filename: String,
    pub size: u64,
}

fn default_exclude_patterns() -> Vec<String> {
    vec![
        "pkg/".to_string(),
        ".git".to_string(),
        ".gitignore".to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tarball_creation() {
        // Test tarball creation
    }

    #[test]
    fn test_exclude_patterns() {
        // Test exclude logic
    }

    #[test]
    fn test_file_inclusion() {
        // Test correct files are included
    }

    #[test]
    fn test_metadata_file_included() {
        // Test METADATA.json is in tarball
    }
}
```

**Checklist**:
- [ ] Implement TarballBuilder
- [ ] Add file traversal
- [ ] Implement exclusion logic
- [ ] Add METADATA.json generation
- [ ] Write 4+ tests

---

### Task 2.2: Integrate with CLI

**File**: `src/cli/build.rs` (Enhanced)

```rust
use crate::builder::ModuleBuilder;
use crate::builder::metadata::ModuleMetadata;
use colored::*;
use std::path::Path;

pub struct BuildCommand;

impl BuildCommand {
    pub fn execute(path: &Path, output: Option<&Path>) -> anyhow::Result<()> {
        println!("{} Building module...", "⚙".cyan());

        // Load metadata
        let metadata_path = path.join("metadata.json");
        let metadata = ModuleMetadata::load(&metadata_path)?;

        println!("  Module: {}", metadata.name.cyan());
        println!("  Version: {}", metadata.version.green());

        // Create output directory
        let output_dir = output.unwrap_or(path);
        let pkg_dir = output_dir.join("pkg");
        std::fs::create_dir_all(&pkg_dir)?;

        // Build
        let builder = ModuleBuilder::new(path, metadata);
        let result = builder.build(&pkg_dir)?;

        println!("{} {}", "✓".green(), result.filename.bright_white());
        println!("  Size: {} bytes", result.size);

        Ok(())
    }
}
```

**Checklist**:
- [ ] Update build.rs with new builder
- [ ] Handle metadata loading
- [ ] Display progress
- [ ] Show build results

---

## Week 3: Advanced Features & Polish

### Task 3.1: Implement Dependency Resolver

**File**: `src/builder/dependency.rs`

```rust
use crate::builder::metadata::ModuleDependency;
use std::collections::{HashMap, HashSet};
use anyhow::{anyhow, Result};

pub struct DependencyResolver {
    dependencies: HashMap<String, Vec<ModuleDependency>>,
}

impl DependencyResolver {
    pub fn new() -> Self {
        Self {
            dependencies: HashMap::new(),
        }
    }

    pub fn add_module(&mut self, name: String, deps: Vec<ModuleDependency>) {
        self.dependencies.insert(name, deps);
    }

    /// Resolve dependency tree
    pub fn resolve(&self, module: &str) -> Result<DependencyTree> {
        let mut tree = DependencyTree::new(module.to_string());
        let mut visited = HashSet::new();
        self.resolve_recursive(module, &mut visited, &mut tree)?;
        Ok(tree)
    }

    fn resolve_recursive(
        &self,
        module: &str,
        visited: &mut HashSet<String>,
        tree: &mut DependencyTree,
    ) -> Result<()> {
        if visited.contains(module) {
            return Err(anyhow!("Circular dependency detected: {}", module));
        }

        visited.insert(module.to_string());

        if let Some(deps) = self.dependencies.get(module) {
            for dep in deps {
                tree.add_dependency(module, &dep.name);
                self.resolve_recursive(&dep.name, &mut visited.clone(), tree)?;
            }
        }

        Ok(())
    }

    /// Check for conflicts
    pub fn check_conflicts(&self) -> Result<()> {
        // Detect version conflicts
        Ok(())
    }
}

pub struct DependencyTree {
    root: String,
    nodes: HashMap<String, Vec<String>>,
}

impl DependencyTree {
    pub fn new(root: String) -> Self {
        Self {
            root,
            nodes: HashMap::new(),
        }
    }

    pub fn add_dependency(&mut self, parent: &str, child: &str) {
        self.nodes
            .entry(parent.to_string())
            .or_insert_with(Vec::new)
            .push(child.to_string());
    }

    pub fn to_string(&self) -> String {
        let mut result = format!("Dependency Tree for: {}\n", self.root);
        for (parent, children) in &self.nodes {
            result.push_str(&format!("  {}\n", parent));
            for child in children {
                result.push_str(&format!("    ├─ {}\n", child));
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dependency_resolution() {}

    #[test]
    fn test_circular_dependency_detection() {}

    #[test]
    fn test_version_conflict_detection() {}
}
```

**Checklist**:
- [ ] Implement resolver
- [ ] Add circular dependency detection
- [ ] Add conflict checking
- [ ] Write 3+ tests

---

### Task 3.2: Add Format Support

**File**: `src/builder/formats.rs`

```rust
pub enum BuildFormat {
    TarGz,
    TarBz2,
    Zip,
}

impl BuildFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            BuildFormat::TarGz => ".tar.gz",
            BuildFormat::TarBz2 => ".tar.bz2",
            BuildFormat::Zip => ".zip",
        }
    }

    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            ".tar.gz" => Some(BuildFormat::TarGz),
            ".tar.bz2" => Some(BuildFormat::TarBz2),
            ".zip" => Some(BuildFormat::Zip),
            _ => None,
        }
    }
}

pub struct FormatBuilder;

impl FormatBuilder {
    pub fn build(
        format: BuildFormat,
        module_path: &Path,
        output_path: &Path,
    ) -> Result<()> {
        match format {
            BuildFormat::TarGz => Self::build_tar_gz(module_path, output_path),
            BuildFormat::TarBz2 => Self::build_tar_bz2(module_path, output_path),
            BuildFormat::Zip => Self::build_zip(module_path, output_path),
        }
    }

    fn build_tar_gz(module_path: &Path, output_path: &Path) -> Result<()> {
        // Implementation
        Ok(())
    }

    fn build_tar_bz2(module_path: &Path, output_path: &Path) -> Result<()> {
        // Implementation
        Ok(())
    }

    fn build_zip(module_path: &Path, output_path: &Path) -> Result<()> {
        // Implementation
        Ok(())
    }
}
```

**Checklist**:
- [ ] Implement tar.gz format
- [ ] Implement tar.bz2 format
- [ ] Implement ZIP format
- [ ] Write tests (3+ tests)

---

## Summary: Week-by-Week Breakdown

### Week 1
```
src/builder/metadata.rs
- ModuleMetadata struct
- Validation logic
- Version bumping
- Tests: 8

src/builder/checksum.rs
- SHA256 hashing
- MD5 hashing
- Tests: 2

Total: 10 tests, ~400 lines Rust
```

### Week 2
```
src/builder/packager.rs
- TarballBuilder
- File traversal
- Exclusion patterns
- METADATA.json generation
- Tests: 4

src/cli/build.rs (Enhanced)
- CLI integration
- Progress reporting
- Tests: 3

src/builder/mod.rs
- Public API
- Tests: 2

Total: 9 tests, ~300 lines Rust
```

### Week 3
```
src/builder/dependency.rs
- DependencyResolver
- Circular detection
- Conflict checking
- Tests: 3

src/builder/formats.rs
- tar.gz format
- tar.bz2 format
- ZIP format
- Tests: 3

Polish & Documentation
- Update docs
- Integration tests
- Tests: 5+

Total: 11+ tests, ~250 lines Rust
```

**Grand Total Phase 1**:
- 30+ unit tests
- ~950 lines Rust code
- Complete build functionality
- Full documentation

---

## Testing Strategy

### Unit Tests
- Each module: 3-4 tests
- Edge cases coverage
- Error handling

### Integration Tests
- Real module builds
- Checksum verification
- Metadata validation

### Acceptance Tests
- Forge compatibility
- Performance benchmarks

---

## Performance Targets

| Operation | Target | Notes |
|-----------|--------|-------|
| Metadata loading | <10ms | In-memory JSON parse |
| Tarball creation | <300ms | For typical 500-file module |
| Checksum generation | <50ms | For 10MB tarball |
| Total build time | <500ms | End-to-end |

---

## Success Criteria Checklist

- [ ] All 30+ tests passing
- [ ] Code coverage >90%
- [ ] Build time <500ms
- [ ] Tarball format matches PDK
- [ ] Metadata validation complete
- [ ] All edge cases handled
- [ ] Documentation complete
- [ ] Code reviewed and approved

---

**Next**: Proceed to Test Phase implementation after this phase is complete.
