//! Regent Phase 4 - Code Generator
//!
//! Advanced component generation for Puppet modules including:
//! - Providers
//! - Deferred functions
//! - Bolt transport plugins

pub mod advanced;

pub use advanced::{
    BoltTransportGenerator, ClassGenerator, ComponentType, DeferredFunctionGenerator,
    GeneratorConfig, ProviderGenerator, ResourceGenerator,
};

/// Module generator trait for integration
pub trait ModuleGenerator: Send + Sync {
    fn generate(&self) -> anyhow::Result<std::path::PathBuf>;
}
