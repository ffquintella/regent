use colored::*;
use std::path::Path;

use regent::tester::bundled_gems::ensure_bundled_gems;

pub struct BootstrapCommand;

/// Gems Regent's embedded Artichoke Ruby runner needs in order to execute
/// `regent test`. These are sourced from the Regent-shipped gem cache — we do
/// not shell out to a host Ruby or Bundler.
const REQUIRED_GEMS: &[&str] = &["rspec", "rspec-core", "rspec-expectations", "rspec-support"];

impl BootstrapCommand {
    pub fn execute(path: &Path, _force: bool) -> anyhow::Result<()> {
        let module_path = path
            .canonicalize()
            .unwrap_or_else(|_| path.to_path_buf());

        println!(
            "{} Bootstrapping Regent dependencies in {}",
            "⚙".cyan(),
            module_path.display()
        );
        println!(
            "  {} Regent uses its embedded Artichoke Ruby runtime — no host Ruby or Bundler required.",
            "•".cyan()
        );

        match ensure_bundled_gems(&module_path) {
            Ok(Some(src)) => println!(
                "{} Installed Regent-shipped gem cache from {}",
                "✓".green().bold(),
                src.display()
            ),
            Ok(None) => {
                return Err(anyhow::anyhow!(
                    "No Regent-shipped gem cache was found.\n\
                     Set REGENT_BUNDLED_GEMS to a directory containing a `ruby/<x.y.z>/gems/...` layout, \
                     or reinstall Regent from a package that bundles its gem cache."
                ));
            }
            Err(err) => {
                return Err(anyhow::anyhow!(
                    "Could not install shipped gem cache into {}: {err}",
                    module_path.join("vendor").join("bundle").display()
                ));
            }
        }

        verify_required_gems(&module_path)?;

        println!(
            "{} Regent is ready. You can now run `regent test`.",
            "✓".green().bold()
        );
        Ok(())
    }
}

fn verify_required_gems(module_path: &Path) -> anyhow::Result<()> {
    let bundle_root = module_path.join("vendor").join("bundle").join("ruby");
    let mut missing = Vec::new();
    for name in REQUIRED_GEMS {
        if !gem_present(&bundle_root, name) {
            missing.push(*name);
        }
    }
    if !missing.is_empty() {
        return Err(anyhow::anyhow!(
            "Regent's shipped gem cache is missing required gem(s): {}.\n\
             Reinstall Regent from a package that bundles these gems, or point REGENT_BUNDLED_GEMS at a complete cache.",
            missing.join(", ")
        ));
    }
    Ok(())
}

fn gem_present(bundle_root: &Path, gem_name: &str) -> bool {
    let Ok(entries) = std::fs::read_dir(bundle_root) else {
        return false;
    };
    for entry in entries.flatten() {
        let gems_dir = entry.path().join("gems");
        let Ok(gems) = std::fs::read_dir(&gems_dir) else { continue };
        for gem in gems.flatten() {
            if let Some(name) = gem.file_name().to_str() {
                if name.starts_with(&format!("{gem_name}-")) {
                    return true;
                }
            }
        }
    }
    false
}

/// Hint shown by other commands when a Regent dependency is missing at runtime.
pub fn missing_dependency_hint(what: &str) -> String {
    format!(
        "{what} was not found in the embedded Ruby's gem cache.\n\
         Run `regent bootstrap` in your module directory to install Regent's required gems.\n\
         Regent uses its embedded Artichoke Ruby runtime — no host Ruby or Bundler is involved."
    )
}
