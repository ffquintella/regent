//! Advanced component generation for Phase 4
//!
//! Supports generation of:
//! - Custom providers
//! - Deferred functions
//! - Bolt transports
//! - Puppet classes
//! - Resources

pub mod provider;
pub mod deferred;
pub mod bolt;
pub mod class;
pub mod resource;

pub use provider::ProviderGenerator;
pub use deferred::DeferredFunctionGenerator;
pub use bolt::BoltTransportGenerator;
pub use class::ClassGenerator;
pub use resource::ResourceGenerator;

use std::path::PathBuf;
use serde::{Deserialize, Serialize};

/// Component type to generate
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComponentType {
    Provider,
    DeferredFunction,
    BoltTransport,
}

impl std::fmt::Display for ComponentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComponentType::Provider => write!(f, "provider"),
            ComponentType::DeferredFunction => write!(f, "deferred_function"),
            ComponentType::BoltTransport => write!(f, "bolt_transport"),
        }
    }
}

/// Configuration for component generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratorConfig {
    pub module_path: PathBuf,
    pub component_name: String,
    pub component_type: ComponentType,
    pub create_tests: bool,
    pub create_docs: bool,
}

impl GeneratorConfig {
    pub fn new(
        module_path: impl AsRef<std::path::Path>,
        component_name: String,
        component_type: ComponentType,
    ) -> Self {
        Self {
            module_path: module_path.as_ref().to_path_buf(),
            component_name,
            component_type,
            create_tests: true,
            create_docs: true,
        }
    }

    pub fn with_tests(mut self, create_tests: bool) -> Self {
        self.create_tests = create_tests;
        self
    }

    pub fn with_docs(mut self, create_docs: bool) -> Self {
        self.create_docs = create_docs;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component_type_display() {
        assert_eq!(ComponentType::Provider.to_string(), "provider");
        assert_eq!(ComponentType::DeferredFunction.to_string(), "deferred_function");
        assert_eq!(ComponentType::BoltTransport.to_string(), "bolt_transport");
    }

    #[test]
    fn test_generator_config_creation() {
        let config = GeneratorConfig::new(
            ".",
            "my_component".to_string(),
            ComponentType::Provider,
        );
        assert_eq!(config.component_name, "my_component");
        assert_eq!(config.component_type, ComponentType::Provider);
        assert!(config.create_tests);
        assert!(config.create_docs);
    }

    #[test]
    fn test_generator_config_builder() {
        let config = GeneratorConfig::new(".", "test".to_string(), ComponentType::Provider)
            .with_tests(false)
            .with_docs(false);

        assert!(!config.create_tests);
        assert!(!config.create_docs);
    }
}
