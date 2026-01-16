use colored::*;
use std::path::Path;

pub struct ValidateCommand;

impl ValidateCommand {
    pub fn execute(path: &Path) -> anyhow::Result<()> {
        // Check module structure
        let required_dirs = vec!["manifests", "metadata.json"];

        let mut has_errors = false;

        for item in &required_dirs {
            let item_path = path.join(item);
            if !item_path.exists() {
                println!(
                    "{} Missing required: {}",
                    "✗".red(),
                    item
                );
                has_errors = true;
            } else {
                println!("{} Found: {}", "✓".green(), item);
            }
        }

        // Validate metadata.json
        if let Ok(metadata) = std::fs::read_to_string(path.join("metadata.json")) {
            match serde_json::from_str::<serde_json::Value>(&metadata) {
                Ok(_) => println!("{} metadata.json is valid JSON", "✓".green()),
                Err(e) => {
                    println!("{} Invalid metadata.json: {}", "✗".red(), e);
                    has_errors = true;
                }
            }
        }

        if has_errors {
            Err(anyhow::anyhow!("Validation failed"))
        } else {
            println!("\n{} Module validation passed!", "✓".green().bold());
            Ok(())
        }
    }
}
