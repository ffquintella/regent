//! Deferred function generator for Phase 4

use crate::generator::advanced::GeneratorConfig;
use anyhow::Result;
use std::fs;
use std::path::PathBuf;

/// Generates Puppet deferred functions (Puppet 5.5+)
pub struct DeferredFunctionGenerator {
    config: GeneratorConfig,
}

impl DeferredFunctionGenerator {
    pub fn new(config: &GeneratorConfig) -> Result<Self> {
        Ok(Self {
            config: config.clone(),
        })
    }

    pub fn generate(&self) -> Result<PathBuf> {
        let functions_dir = self.config.module_path.join("lib/puppet/functions");
        fs::create_dir_all(&functions_dir)?;

        let file_name = format!("{}.rb", self.config.component_name);
        let file_path = functions_dir.join(&file_name);

        let content = self.generate_function_content();
        fs::write(&file_path, content)?;

        if self.config.create_tests {
            self.generate_tests()?;
        }

        if self.config.create_docs {
            self.generate_docs()?;
        }

        Ok(file_path)
    }

    fn generate_function_content(&self) -> String {
        let func_name = self.config.component_name.as_str();
        let module_name = self.config.module_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("module");

        format!(
            "# Deferred function: {}::{}\nPuppet::Functions.create_function('{}::{}') do\nend\n",
            module_name, func_name, module_name, func_name
        )
    }

    fn generate_tests(&self) -> Result<()> {
        let spec_dir = self.config.module_path.join("spec/functions");
        fs::create_dir_all(&spec_dir)?;

        let test_file = spec_dir.join(format!("{}_spec.rb", self.config.component_name));
        let test_content = "require 'spec_helper'\n";

        fs::write(test_file, test_content)?;
        Ok(())
    }

    fn generate_docs(&self) -> Result<()> {
        let docs_dir = self.config.module_path.join("docs");
        fs::create_dir_all(&docs_dir)?;

        let doc_file = docs_dir.join(format!("function_{}.md", self.config.component_name));
        let doc_content = format!("# Deferred Function: {}\n", self.config.component_name);

        fs::write(doc_file, doc_content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_deferred_function_generator_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = GeneratorConfig::new(
            temp_dir.path(),
            "resolve_value".to_string(),
            crate::generator::advanced::ComponentType::DeferredFunction,
        );
        let result = DeferredFunctionGenerator::new(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_deferred_function_generates_file() {
        let temp_dir = TempDir::new().unwrap();
        let config = GeneratorConfig::new(
            temp_dir.path(),
            "resolve_value".to_string(),
            crate::generator::advanced::ComponentType::DeferredFunction,
        )
        .with_tests(false)
        .with_docs(false);

        let generator = DeferredFunctionGenerator::new(&config).unwrap();
        let result = generator.generate();
        assert!(result.is_ok());
        assert!(result.unwrap().exists());
    }

    #[test]
    fn test_deferred_function_content_has_puppet_functions() {
        let temp_dir = TempDir::new().unwrap();
        let config = GeneratorConfig::new(
            temp_dir.path(),
            "resolve_value".to_string(),
            crate::generator::advanced::ComponentType::DeferredFunction,
        )
        .with_tests(false)
        .with_docs(false);

        let generator = DeferredFunctionGenerator::new(&config).unwrap();
        let file_path = generator.generate().unwrap();
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("Puppet::Functions.create_function"));
    }

    #[test]
    fn test_deferred_function_generates_tests() {
        let temp_dir = TempDir::new().unwrap();
        let config = GeneratorConfig::new(
            temp_dir.path(),
            "resolve_value".to_string(),
            crate::generator::advanced::ComponentType::DeferredFunction,
        )
        .with_tests(true)
        .with_docs(false);

        let generator = DeferredFunctionGenerator::new(&config).unwrap();
        let _ = generator.generate();
        let test_file = temp_dir.path().join("spec/functions/resolve_value_spec.rb");
        assert!(test_file.exists());
    }

    #[test]
    fn test_deferred_function_generates_docs() {
        let temp_dir = TempDir::new().unwrap();
        let config = GeneratorConfig::new(
            temp_dir.path(),
            "resolve_value".to_string(),
            crate::generator::advanced::ComponentType::DeferredFunction,
        )
        .with_tests(false)
        .with_docs(true);

        let generator = DeferredFunctionGenerator::new(&config).unwrap();
        let _ = generator.generate();
        let doc_file = temp_dir.path().join("docs/function_resolve_value.md");
        assert!(doc_file.exists());
    }
}
