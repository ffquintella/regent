use colored::*;
use std::path::Path;

use regent::builder::packager::{PackagerConfig, TarballBuilder};

pub struct BuildCommand;

impl BuildCommand {
    pub fn execute(path: &Path, output: Option<&Path>) -> anyhow::Result<()> {
        let metadata_path = path.join("metadata.json");
        if !metadata_path.exists() {
            return Err(anyhow::anyhow!("No module found at {:?}", path));
        }

        // Anchor everything at the canonical module path so relative invocations
        // (e.g. `regent build .`) always end up in the module's own pkg/ directory.
        let module_path = path
            .canonicalize()
            .unwrap_or_else(|_| path.to_path_buf());

        let metadata = std::fs::read_to_string(&metadata_path)?;
        let json: serde_json::Value = serde_json::from_str(&metadata)?;
        let module_name = json["name"].as_str().unwrap_or("module");
        let version = json["version"].as_str().unwrap_or("0.1.0");

        // Resolve the output directory:
        // - explicit `--output`: respect as-is (absolute) or anchor to the module
        //   if relative, instead of the user's cwd.
        // - default: <module_path>/pkg
        let output_dir = match output {
            Some(out) if out.is_absolute() => out.to_path_buf(),
            Some(out) => module_path.join(out),
            None => module_path.join("pkg"),
        };

        let mut config = PackagerConfig::new(&module_path);
        config = config.with_output_dir(&output_dir);

        let builder = TarballBuilder::new(config)?;

        println!(
            "{} Building {}-{} → {}",
            "⚙".cyan(),
            module_name,
            version,
            output_dir.display()
        );

        let package_path = builder.build(module_name, version)?;

        println!(
            "{} Module '{}' built successfully: {}",
            "✓".green().bold(),
            module_name,
            package_path.display()
        );

        Ok(())
    }
}
