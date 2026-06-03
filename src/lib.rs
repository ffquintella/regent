// Regent - Rust + Artichoke Ruby hybrid module for Puppet development

pub mod builder;
pub mod config;
pub mod generator;
pub mod ruby_interop;
pub mod tester;
pub mod validator;

pub use builder::{
    BuildArtifact, BuildFormat, ChecksumGenerator, DependencyResolver, ModuleBuilder,
    ModuleMetadata, PackagerConfig, TarballBuilder,
};
pub use config::Config;
pub use generator::ModuleGenerator;
pub use tester::{ModuleTester, TestConfig, TestResults, TestType};
pub use validator::ModuleValidator;

/// Regent version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Main Regent API
pub struct Regent {
    config: Config,
}

impl Regent {
    pub fn new() -> Self {
        Self {
            config: Config::default(),
        }
    }

    pub fn configure<F>(mut self, f: F) -> Self
    where
        F: FnOnce(&mut Config),
    {
        f(&mut self.config);
        self
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn config_mut(&mut self) -> &mut Config {
        &mut self.config
    }
}

impl Default for Regent {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regent_creation() {
        let regent = Regent::new();
        assert_eq!(regent.config.project_name, None);
    }
}
