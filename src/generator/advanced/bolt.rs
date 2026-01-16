//! Bolt transport plugin generator for Phase 4

use crate::generator::advanced::GeneratorConfig;
use anyhow::Result;
use std::fs;
use std::path::PathBuf;

/// Generates Puppet Bolt transport plugins
pub struct BoltTransportGenerator {
    config: GeneratorConfig,
}

impl BoltTransportGenerator {
    pub fn new(config: &GeneratorConfig) -> Result<Self> {
        Ok(Self {
            config: config.clone(),
        })
    }

    pub fn generate(&self) -> Result<PathBuf> {
        let transports_dir = self.config.module_path.join("lib/puppet_x/bolt/transport");
        fs::create_dir_all(&transports_dir)?;

        let file_name = format!("{}.rb", self.config.component_name);
        let file_path = transports_dir.join(&file_name);

        let content = self.generate_transport_content();
        fs::write(&file_path, content)?;

        if self.config.create_tests {
            self.generate_tests()?;
        }

        if self.config.create_docs {
            self.generate_docs()?;
        }

        Ok(file_path)
    }

    fn generate_transport_content(&self) -> String {
        let transport_name = self.config.component_name.as_str();
        format!(
            "# Bolt Transport: {}\nmodule Bolt\n  module Transport\n    class {} < Base\n    end\n  end\nend\n",
            transport_name, transport_name
        )
    }

    fn generate_tests(&self) -> Result<()> {
        let spec_dir = self.config.module_path.join("spec/transport");
        fs::create_dir_all(&spec_dir)?;

        let test_file = spec_dir.join(format!("{}_spec.rb", self.config.component_name));
        let test_content = "require 'spec_helper'\n";

        fs::write(test_file, test_content)?;
        Ok(())
    }

    fn generate_docs(&self) -> Result<()> {
        let docs_dir = self.config.module_path.join("docs");
        fs::create_dir_all(&docs_dir)?;

        let doc_file = docs_dir.join(format!("transport_{}.md", self.config.component_name));
        let doc_content = format!("# Bolt Transport: {}\n", self.config.component_name);

        fs::write(doc_file, doc_content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_bolt_transport_generator_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = GeneratorConfig::new(
            temp_dir.path(),
            "ssh_custom".to_string(),
            crate::generator::advanced::ComponentType::BoltTransport,
        );
        let result = BoltTransportGenerator::new(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_bolt_transport_generates_file() {
        let temp_dir = TempDir::new().unwrap();
        let config = GeneratorConfig::new(
            temp_dir.path(),
            "ssh_custom".to_string(),
            crate::generator::advanced::ComponentType::BoltTransport,
        )
        .with_tests(false)
        .with_docs(false);

        let generator = BoltTransportGenerator::new(&config).unwrap();
        let result = generator.generate();
        assert!(result.is_ok());
        assert!(result.unwrap().exists());
    }

    #[test]
    fn test_bolt_transport_content_has_module() {
        let temp_dir = TempDir::new().unwrap();
        let config = GeneratorConfig::new(
            temp_dir.path(),
            "ssh_custom".to_string(),
            crate::generator::advanced::ComponentType::BoltTransport,
        )
        .with_tests(false)
        .with_docs(false);

        let generator = BoltTransportGenerator::new(&config).unwrap();
        let file_path = generator.generate().unwrap();
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("module Bolt"));
    }

    #[test]
    fn test_bolt_transport_generates_tests() {
        let temp_dir = TempDir::new().unwrap();
        let config = GeneratorConfig::new(
            temp_dir.path(),
            "ssh_custom".to_string(),
            crate::generator::advanced::ComponentType::BoltTransport,
        )
        .with_tests(true)
        .with_docs(false);

        let generator = BoltTransportGenerator::new(&config).unwrap();
        let _ = generator.generate();
        let test_file = temp_dir.path().join("spec/transport/ssh_custom_spec.rb");
        assert!(test_file.exists());
    }

    #[test]
    fn test_bolt_transport_generates_docs() {
        let temp_dir = TempDir::new().unwrap();
        let config = GeneratorConfig::new(
            temp_dir.path(),
            "ssh_custom".to_string(),
            crate::generator::advanced::ComponentType::BoltTransport,
        )
        .with_tests(false)
        .with_docs(true);

        let generator = BoltTransportGenerator::new(&config).unwrap();
        let _ = generator.generate();
        let doc_file = temp_dir.path().join("docs/transport_ssh_custom.md");
        assert!(doc_file.exists());
    }
}
