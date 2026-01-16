use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub project_name: Option<String>,
    pub author: Option<String>,
    pub license: Option<String>,
    pub description: Option<String>,
    pub module_path: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            project_name: None,
            author: None,
            license: Some("Apache-2.0".to_string()),
            description: None,
            module_path: None,
        }
    }
}

impl Config {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_name(mut self, name: String) -> Self {
        self.project_name = Some(name);
        self
    }

    pub fn with_author(mut self, author: String) -> Self {
        self.author = Some(author);
        self
    }

    pub fn with_license(mut self, license: String) -> Self {
        self.license = Some(license);
        self
    }
}
