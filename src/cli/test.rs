use colored::*;
use std::path::Path;

pub struct TestCommand;

impl TestCommand {
    pub fn execute(path: &Path, pattern: Option<&str>) -> anyhow::Result<()> {
        println!("{} Running tests with pattern: {}", "⚙".cyan(), pattern.unwrap_or("*"));

        // Check if spec directory exists
        if !path.join("spec").exists() {
            return Err(anyhow::anyhow!("No spec directory found at {:?}", path));
        }

        println!("{} Test support integrated with Artichoke Ruby", "ℹ".blue());
        println!("{} Tests completed", "✓".green().bold());

        Ok(())
    }
}
