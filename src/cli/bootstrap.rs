use colored::*;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use regent::tester::bundled_gems::{ensure_user_bundle, user_bundle_dir};

pub struct BootstrapCommand;

/// Gems Regent's embedded Artichoke Ruby runner needs in order to execute
/// `regent test`. These are sourced from the Regent-shipped gem cache — we do
/// not shell out to a host Ruby or Bundler.
const REQUIRED_GEMS: &[&str] = &["rspec", "rspec-core", "rspec-expectations", "rspec-support"];

impl BootstrapCommand {
    pub fn execute(_path: &Path, _force: bool) -> anyhow::Result<()> {
        let Some(bundle_dir) = user_bundle_dir() else {
            return Err(anyhow::anyhow!(
                "Could not determine the per-user Regent bundle dir (is $HOME set?)."
            ));
        };

        println!(
            "{} Bootstrapping Regent into {}",
            "⚙".cyan(),
            bundle_dir.display()
        );
        println!(
            "  {} Regent uses its embedded Artichoke Ruby runtime — no host Ruby or Bundler required.",
            "•".cyan()
        );

        match ensure_user_bundle() {
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
                    bundle_dir.display()
                ));
            }
        }

        verify_required_gems(&bundle_dir)?;

        update_shell_profiles(&bundle_dir);

        println!(
            "{} Regent is ready. You can now run `regent test` in any module directory.",
            "✓".green().bold()
        );
        println!(
            "  {} To activate REGENT_BUNDLED_GEMS in this shell, run: \
             {}",
            "•".cyan(),
            format!("export REGENT_BUNDLED_GEMS={}", bundle_dir.display()).bold()
        );
        Ok(())
    }
}

fn verify_required_gems(bundle_dir: &Path) -> anyhow::Result<()> {
    let bundle_root = bundle_dir.join("ruby");
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

/// Append `export REGENT_BUNDLED_GEMS=<bundle_dir>` to each shell rc file
/// found in $HOME, guarded by a marker line so we never duplicate it.
fn update_shell_profiles(bundle_dir: &Path) {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return;
    };

    let marker = "# >>> regent bundle <<<";
    let block = format!(
        "\n{marker}\nexport REGENT_BUNDLED_GEMS=\"{path}\"\n# <<< regent bundle >>>\n",
        path = bundle_dir.display()
    );

    for rc in [".zshrc", ".bashrc", ".bash_profile", ".profile"] {
        let rc_path = home.join(rc);
        if !rc_path.exists() {
            continue;
        }
        match std::fs::read_to_string(&rc_path) {
            Ok(content) if content.contains(marker) => {
                // Already configured.
            }
            Ok(_) => {
                if let Ok(mut file) = OpenOptions::new().append(true).open(&rc_path) {
                    if file.write_all(block.as_bytes()).is_ok() {
                        println!(
                            "{} Updated {} to export REGENT_BUNDLED_GEMS",
                            "✓".green().bold(),
                            rc_path.display()
                        );
                    }
                }
            }
            Err(_) => {}
        }
    }
}

/// Hint shown by other commands when a Regent dependency is missing at runtime.
pub fn missing_dependency_hint(what: &str) -> String {
    format!(
        "{what} was not found in the embedded Ruby's gem cache.\n\
         Run `regent bootstrap` to install Regent's required gems into ~/.regent/bundle.\n\
         Regent uses its embedded Artichoke Ruby runtime — no host Ruby or Bundler is involved."
    )
}
