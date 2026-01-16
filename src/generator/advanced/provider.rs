//! Provider generator for Phase 4

use crate::generator::advanced::GeneratorConfig;
use anyhow::Result;
use std::fs;
use std::path::PathBuf;

/// Generates Puppet provider implementations
pub struct ProviderGenerator {
    config: GeneratorConfig,
}

impl ProviderGenerator {
    pub fn new(config: &GeneratorConfig) -> Result<Self> {
        Ok(Self {
            config: config.clone(),
        })
    }

    pub fn generate(&self) -> Result<PathBuf> {
        let providers_dir = self.config.module_path.join("lib/puppet/provider");
        fs::create_dir_all(&providers_dir)?;

        let file_name = format!("{}.rb", self.config.component_name);
        let file_path = providers_dir.join(&file_name);

        let content = self.generate_provider_content();
        fs::write(&file_path, content)?;

        if self.config.create_tests {
            self.generate_tests()?;
        }

        if self.config.create_docs {
            self.generate_docs()?;
        }

        Ok(file_path)
    }

    fn generate_provider_content(&self) -> String {
        let provider_name = self.config.component_name.as_str();
        format!(
            "# Provider: {}\nPuppet::Type.type(:{}).provide(:unix) do\nend\n",
            provider_name, provider_name
        )
    }

    fn generate_tests(&self) -> Result<()> {
        let spec_dir = self.config.module_path.join("spec/providers");
        fs::create_dir_all(&spec_dir)?;

        let test_file = spec_dir.join(format!("{}_spec.rb", self.config.component_name));
        let test_content = "require 'spec_helper'\n";

        fs::write(test_file, test_content)?;
        Ok(())
    }

    fn generate_docs(&self) -> Result<()> {
        let docs_dir = self.config.module_path.join("docs");
        fs::create_dir_all(&docs_dir)?;

        let doc_file = docs_dir.join(format!("provider_{}.md", self.config.component_name));
        let doc_content = format!("# Provider: {}\n", self.config.component_name);

        fs::write(doc_file, doc_content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_provider_generator_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = GeneratorConfig::new(
            temp_dir.path(),
            "service".to_string(),
            crate::generator::advanced::ComponentType::Provider,
        );
        let result = ProviderGenerator::new(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_provider_generates_file() {
        let temp_dir = TempDir::new().unwrap();
        let config = GeneratorConfig::new(
            temp_dir.path(),
            "service".to_string(),
            crate::generator::advanced::ComponentType::Provider,
        )
        .with_tests(false)
        .with_docs(false);

        let generator = ProviderGenerator::new(&config).unwrap();
        let result = generator.generate();
        assert!(result.is_ok());
        assert!(result.unwrap().exists());
    }

    #[test]
    fn test_provider_content_has_type() {
        let temp_dir = TempDir::new().unwrap();
        let config = GeneratorConfig::new(
            temp_dir.path(),
            "service".to_string(),
            crate::generator::advanced::ComponentType::Provider,
        )
        .with_tests(false)
        .with_docs(false);

        let generator = ProviderGenerator::new(&config).unwrap();
        let file_path = generator.generate().unwrap();
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("Puppet::Type"));
    }

    #[test]
    fn test_provider_generates_tests() {
        let temp_dir = TempDir::new().unwrap();
        let config = GeneratorConfig::new(
            temp_dir.path(),
            "service".to_string(),
            crate::generator::advanced::ComponentType::Provider,
        )
        .with_tests(true)
        .with_docs(false);

        let generator = ProviderGenerator::new(&config).unwrap();
        let _ = generator.generate();
        let test_file = temp_dir.path().join("spec/providers/service_spec.rb");
        assert!(test_file.exists());
    }

    #[test]
    fn test_provider_generates_docs() {
        let temp_dir = TempDir::new().unwrap();
        let config = GeneratorConfig::new(
            temp_dir.path(),
            "service".to_string(),
            crate::generator::advanced::ComponentType::Provider,
        )
        .with_tests(false)
        .with_docs(true);

        let generator = ProviderGenerator::new(&config).unwrap();
        let _ = generator.generate();
        let doc_file = temp_dir.path().join("docs/provider_service.md");
        assert!(doc_file.exists());
    }
}
