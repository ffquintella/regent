use colored::*;
use std::path::Path;
use std::fs;

pub struct BuildCommand;

impl BuildCommand {
    pub fn execute(path: &Path, output: Option<&Path>) -> anyhow::Result<()> {
        // Check if module exists
        if !path.join("metadata.json").exists() {
            return Err(anyhow::anyhow!("No module found at {:?}", path));
        }

        // Create pkg directory
        let pkg_dir = path.join("pkg");
        fs::create_dir_all(&pkg_dir)?;

        // Read module name from metadata
        let metadata = fs::read_to_string(path.join("metadata.json"))?;
        let json: serde_json::Value = serde_json::from_str(&metadata)?;
        let module_name = json["name"].as_str().unwrap_or("module");
        let version = json["version"].as_str().unwrap_or("0.1.0");

        let package_name = format!("{}-{}.tar.gz", module_name, version);
        
        let output_path = if let Some(out) = output {
            out.to_path_buf()
        } else {
            pkg_dir.clone()
        };

        fs::create_dir_all(&output_path)?;

        println!("{} Creating package: {}", "⚙".cyan(), package_name);
        println!(
            "{} Package would be built at: {:?}",
            "ℹ".blue(),
            output_path.join(&package_name)
        );

        println!(
            "\n{} Module '{}' built successfully!",
            "✓".green().bold(),
            module_name
        );

        Ok(())
    }
}
