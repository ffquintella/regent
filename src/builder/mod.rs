pub mod metadata;
pub mod checksum;
pub mod packager;
pub mod dependency;

pub use metadata::ModuleMetadata;
pub use checksum::{ChecksumGenerator, ChecksumSet};
pub use packager::{TarballBuilder, PackagerConfig, BuildFormat};
pub use dependency::{DependencyResolver, ModuleDependency, DependencyTree};

use std::path::{Path, PathBuf};
use anyhow::{Context, Result};

/// Artifact produced by building a module
#[derive(Debug, Clone)]
pub struct BuildArtifact {
    /// Path to the generated tarball
    pub tarball_path: PathBuf,
    /// SHA256 checksum of the tarball
    pub sha256: String,
    /// MD5 checksum of the tarball
    pub md5: String,
    /// Module name (author-modulename)
    pub module_name: String,
    /// Module version
    pub version: String,
}

pub struct ModuleBuilder;

impl ModuleBuilder {
    /// Build a module package with all artifacts
    pub fn build(
        path: &Path,
        output: Option<&Path>,
        version_override: Option<&str>,
    ) -> Result<BuildArtifact> {
        // Validate module first
        crate::validator::Validator::validate(path)?;

        // Load and validate metadata
        let metadata_path = path.join("metadata.json");
        let mut metadata = ModuleMetadata::load(&metadata_path)
            .context("Failed to load metadata.json")?;
        
        metadata.validate()
            .context("Metadata validation failed")?;

        // Apply version override if provided
        if let Some(version) = version_override {
            metadata.version = version.to_string();
            metadata.validate()?; // Re-validate with new version
        }

        let version = metadata.version.clone();
        let module_name = metadata.name.clone();

        // Configure packager
        let mut config = PackagerConfig::new(path);
        if let Some(out) = output {
            config = config.with_output_dir(out);
        }

        // Build tarball
        let builder = TarballBuilder::new(config)
            .context("Failed to create tarball builder")?;
        
        let tarball_path = builder.build(&module_name, &version)
            .context("Failed to build tarball")?;

        // Generate checksums
        let checksum_gen = ChecksumGenerator::new(&tarball_path);
        let checksums = checksum_gen.generate_all()
            .context("Failed to generate checksums")?;

        log::info!("Built module: {:?}", tarball_path);
        log::info!("SHA256: {}", checksums.sha256);
        log::info!("MD5: {}", checksums.md5);

        Ok(BuildArtifact {
            tarball_path,
            sha256: checksums.sha256,
            md5: checksums.md5,
            module_name,
            version,
        })
    }
}

