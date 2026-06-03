use colored::*;
use std::path::Path;

use regent::tester::FixtureManager;

pub struct FixturesCommand;

impl FixturesCommand {
    pub fn execute(path: &Path, clean: bool, offline: bool) -> anyhow::Result<()> {
        let module_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let fixtures_yml = module_path.join(".fixtures.yml");
        if !fixtures_yml.exists() {
            return Err(anyhow::anyhow!(
                "No .fixtures.yml found in {}",
                module_path.display()
            ));
        }
        let fixtures_dir = module_path.join("spec").join("fixtures").join("modules");

        let mut manager = FixtureManager::new(&module_path, &fixtures_dir);
        manager.set_offline(offline);
        manager.parse_fixtures_yml(&fixtures_yml)?;

        if clean {
            let removed = manager.cleanup()?;
            if removed > 0 {
                println!(
                    "{} Removed {} existing fixture module(s)",
                    "✓".green().bold(),
                    removed
                );
            }
        }

        let count = manager.setup_fixtures()?;
        let mode = if offline {
            " (offline, from cache)"
        } else {
            ""
        };
        println!(
            "{} Installed {} fixture module(s) into {}{}",
            "✓".green().bold(),
            count,
            fixtures_dir.display(),
            mode
        );
        Ok(())
    }
}
