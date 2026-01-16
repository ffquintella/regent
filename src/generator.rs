use crate::config::Config;
use std::fs;
use std::path::Path;

pub struct ModuleGenerator {
    config: Config,
}

impl ModuleGenerator {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub fn generate(&self, path: &Path) -> anyhow::Result<()> {
        let _module_name = self
            .config
            .project_name
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Project name not set"))?;

        // Create directory structure
        let dirs = vec![
            "manifests",
            "files",
            "templates",
            "tasks",
            "plans",
            "lib",
            "spec",
        ];

        for dir in dirs {
            fs::create_dir_all(path.join(dir))?;
        }

        // Generate metadata.json
        let metadata = self.generate_metadata();
        fs::write(path.join("metadata.json"), metadata)?;

        Ok(())
    }

    fn generate_metadata(&self) -> String {
        serde_json::json!({
            "name": self.config.project_name,
            "version": "0.1.0",
            "author": self.config.author,
            "license": self.config.license,
            "description": self.config.description,
        })
        .to_string()
    }
}
