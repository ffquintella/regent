use colored::*;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use regent::tester::bundled_gems::{ensure_user_bundle, user_bundle_dir};

pub struct BootstrapCommand;

/// Gems Regent's embedded Artichoke Ruby runner needs in order to execute
/// `regent test`. These are sourced from the Regent-shipped gem cache — we do
/// not shell out to a host Ruby or Bundler.
const REQUIRED_GEMS: &[&str] = &[
    "rspec",
    "rspec-core",
    "rspec-expectations",
    "rspec-support",
    "rspec-puppet",
    "rspec-puppet-facts",
    "facterdb",
    "deep_merge",
];

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

        persist_env_var(&bundle_dir);

        println!(
            "{} Regent is ready. You can now run `regent test` in any module directory.",
            "✓".green().bold()
        );
        if cfg!(windows) {
            println!(
                "  {} To activate REGENT_BUNDLED_GEMS in this shell, run: {}",
                "•".cyan(),
                format!("set REGENT_BUNDLED_GEMS={}", bundle_dir.display()).bold()
            );
            println!(
                "    (or open a new shell — `setx` has already updated the persistent value.)"
            );
        } else {
            println!(
                "  {} To activate REGENT_BUNDLED_GEMS in this shell, run: {}",
                "•".cyan(),
                format!("export REGENT_BUNDLED_GEMS={}", bundle_dir.display()).bold()
            );
        }
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
        let Ok(gems) = std::fs::read_dir(&gems_dir) else {
            continue;
        };
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

/// Persist REGENT_BUNDLED_GEMS for future shells.
///
/// On Unix/macOS: append a guarded `export REGENT_BUNDLED_GEMS=…` block to
/// every shell rc file found under `$HOME`.
/// On Windows: use `setx` to write the value to the user-level environment.
fn persist_env_var(bundle_dir: &Path) {
    if cfg!(windows) {
        persist_env_var_windows(bundle_dir);
    } else {
        persist_env_var_unix(bundle_dir);
    }
}

fn persist_env_var_unix(bundle_dir: &Path) {
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
            Ok(content) if content.contains(marker) => {}
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

fn persist_env_var_windows(bundle_dir: &Path) {
    let value = bundle_dir.display().to_string();
    let result = Command::new("setx")
        .arg("REGENT_BUNDLED_GEMS")
        .arg(&value)
        .status();
    match result {
        Ok(status) if status.success() => {
            println!(
                "{} Set user environment variable REGENT_BUNDLED_GEMS via `setx`",
                "✓".green().bold()
            );
        }
        Ok(status) => {
            eprintln!(
                "{} `setx` returned exit code {:?}; set REGENT_BUNDLED_GEMS={} manually.",
                "!".yellow(),
                status.code(),
                value
            );
        }
        Err(err) => {
            eprintln!(
                "{} could not invoke `setx` ({err}); set REGENT_BUNDLED_GEMS={} manually.",
                "!".yellow(),
                value
            );
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
