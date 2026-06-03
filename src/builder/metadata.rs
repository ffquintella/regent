use anyhow::{anyhow, Result};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleDependency {
    pub name: String,
    pub version_requirement: String, // e.g., ">= 1.0.0"
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
        if !self.name.contains('-') && !self.name.contains("::") {
            return Err(anyhow!("Module name must contain '-' or '::'"));
        }

        // Validate version format (semver)
        Version::parse(&self.version).map_err(|e| anyhow!("Invalid version format: {}", e))?;

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
        assert_eq!(metadata.get_module_filename(), "user-mymodule-1.0.0");
    }

    #[test]
    fn test_get_puppet_requirement() {
        let metadata = create_test_metadata();
        let req = metadata.get_puppet_requirement().unwrap();
        assert_eq!(req, ">= 6.0.0");
    }
}
