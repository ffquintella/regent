use anyhow::{Context, Result};
use fs_extra::dir::{copy as copy_dir, CopyOptions};
use fs_extra::file::{copy as copy_file, CopyOptions as FileCopyOptions};
use std::path::{Path, PathBuf};

const BUNDLED_GEMS_DIRNAME: &str = "bundled_gems";
const EMBEDDED_GEM_CACHE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/bundled_gems/cache.tar.gz"
));

/// Gem names and Ruby entrypoints required by the embedded test runner.
/// Keep bootstrap and test preflight checks tied to the same cache contract.
pub const REQUIRED_GEMS: &[(&str, &str)] = &[
    ("rspec", "lib/rspec.rb"),
    ("rspec-core", "lib/rspec/core.rb"),
    ("rspec-expectations", "lib/rspec/expectations.rb"),
    ("rspec-support", "lib/rspec/support.rb"),
    ("rspec-mocks", "lib/rspec/mocks.rb"),
    ("diff-lcs", "lib/diff/lcs.rb"),
    ("rspec-puppet", "lib/rspec-puppet.rb"),
    ("rspec-puppet-facts", "lib/rspec-puppet-facts.rb"),
    ("facterdb", "lib/facterdb.rb"),
    ("deep_merge", "lib/deep_merge.rb"),
];

/// Required gems absent from a Bundler cache, including missing entrypoints.
pub fn missing_required_gems(bundle: &Path) -> Vec<&'static str> {
    REQUIRED_GEMS
        .iter()
        .filter_map(|&(name, entrypoint)| (!gem_present(bundle, name, entrypoint)).then_some(name))
        .collect()
}

fn gem_present(bundle: &Path, gem_name: &str, entrypoint: &str) -> bool {
    let Ok(ruby_versions) = std::fs::read_dir(bundle.join("ruby")) else {
        return false;
    };
    for version in ruby_versions.flatten() {
        let Ok(gems) = std::fs::read_dir(version.path().join("gems")) else {
            continue;
        };
        for gem in gems.flatten() {
            let filename = gem.file_name();
            let Some(name) = filename.to_str() else {
                continue;
            };
            // A version suffix starts with a digit; rspec-core is not rspec.
            let matches = name
                .strip_prefix(gem_name)
                .and_then(|suffix| suffix.strip_prefix('-'))
                .is_some_and(|version| version.starts_with(|c: char| c.is_ascii_digit()));
            if matches && gem.path().join(entrypoint).is_file() {
                return true;
            }
        }
    }
    false
}

/// Verify a cache before bootstrap declares success.
pub fn verify_required_gems(bundle: &Path) -> Result<()> {
    let missing = missing_required_gems(bundle);
    anyhow::ensure!(
        missing.is_empty(),
        "Regent's gem cache is missing required gem(s) or entrypoints: {}.\n\
         Run `regent bootstrap` with a complete Regent-shipped cache, or reinstall Regent from a package that bundles these gems.",
        missing.join(", ")
    );
    Ok(())
}

/// Per-user Regent bundle directory.
///
/// Layout:
/// - Unix / macOS: `$HOME/.regent/bundle`
/// - Windows: `%APPDATA%\Regent\bundle` (falls back to `%LOCALAPPDATA%\Regent\bundle`,
///   then `%USERPROFILE%\.regent\bundle`).
///
/// This is the canonical location where `regent bootstrap` installs gems and
/// where the embedded Artichoke runner looks for them at test time. Sharing
/// one cache across all modules avoids per-module copies and keeps Regent
/// self-contained (no host Ruby/Bundler involvement).
pub fn user_bundle_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            return Some(PathBuf::from(appdata).join("Regent").join("bundle"));
        }
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            return Some(PathBuf::from(local).join("Regent").join("bundle"));
        }
        if let Some(profile) = std::env::var_os("USERPROFILE") {
            return Some(PathBuf::from(profile).join(".regent").join("bundle"));
        }
        None
    } else {
        home_dir().map(|h| h.join(".regent").join("bundle"))
    }
}

/// Per-user Regent fixture cache directory.
///
/// Layout mirrors [`user_bundle_dir`]:
/// - Unix / macOS: `$HOME/.regent/fixtures`
/// - Windows: `%APPDATA%\Regent\fixtures` (falls back to
///   `%LOCALAPPDATA%\Regent\fixtures`, then `%USERPROFILE%\.regent\fixtures`).
///
/// Downloaded Puppet module fixtures (Forge tarballs, git clones) are cached
/// here keyed by source so they can be reused across modules and runs without
/// re-fetching — and, once populated, without any network access at all.
pub fn user_fixtures_dir() -> Option<PathBuf> {
    if let Some(override_dir) = std::env::var_os("REGENT_FIXTURE_CACHE") {
        return Some(PathBuf::from(override_dir));
    }
    if cfg!(windows) {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            return Some(PathBuf::from(appdata).join("Regent").join("fixtures"));
        }
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            return Some(PathBuf::from(local).join("Regent").join("fixtures"));
        }
        if let Some(profile) = std::env::var_os("USERPROFILE") {
            return Some(PathBuf::from(profile).join(".regent").join("fixtures"));
        }
        None
    } else {
        home_dir().map(|h| h.join(".regent").join("fixtures"))
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
}

/// Ensure the per-user bundle (`~/.regent/bundle`) is populated from the
/// Regent-shipped gem cache. Returns the source path that was copied from,
/// or the installed target when the embedded cache is used. Returns `None`
/// only when a per-user bundle location cannot be determined.
pub fn ensure_user_bundle() -> Result<Option<PathBuf>> {
    let Some(target) = user_bundle_dir() else {
        return Ok(None);
    };
    ensure_bundle_at(&target, find_bundled_gems_source)
}

fn ensure_bundle_at(
    target: &Path,
    source: impl FnOnce() -> Result<Option<PathBuf>>,
) -> Result<Option<PathBuf>> {
    // A valid installed cache must remain usable after its installer is removed.
    if missing_required_gems(target).is_empty() {
        return Ok(Some(target.to_path_buf()));
    }
    let Some(source) = source()? else {
        let staged = tempfile::tempdir().context("staging embedded Regent gem cache")?;
        unpack_embedded_cache(staged.path())?;
        install_bundle_from(staged.path(), target)?;
        return Ok(Some(target.to_path_buf()));
    };
    install_bundle_from(&source, target)?;
    Ok(Some(source))
}

fn install_bundle_from(source: &Path, target: &Path) -> Result<()> {
    verify_required_gems(source)?;
    if same_path(source, target) {
        return Ok(());
    }
    std::fs::create_dir_all(target)
        .with_context(|| format!("creating Regent user bundle dir {}", target.display()))?;
    copy_contents_into(source, target).with_context(|| {
        format!(
            "copying gem cache {} -> {}",
            source.display(),
            target.display()
        )
    })?;
    verify_required_gems(target)
}

fn unpack_embedded_cache(target: &Path) -> Result<()> {
    let decoder = flate2::read::GzDecoder::new(EMBEDDED_GEM_CACHE);
    tar::Archive::new(decoder)
        .unpack(target)
        .context("extracting embedded Regent gem cache")?;
    verify_required_gems(target)
}

/// Copy each immediate child of `source` into `target`. fs_extra's
/// `copy_inside = true` does NOT do this — it nests the source dir under the
/// target. Doing it ourselves keeps the layout predictable.
fn copy_contents_into(source: &Path, target: &Path) -> Result<()> {
    let mut dir_opts = CopyOptions::new();
    dir_opts.overwrite = true;
    dir_opts.skip_exist = false;
    dir_opts.copy_inside = false;

    let mut file_opts = FileCopyOptions::new();
    file_opts.overwrite = true;
    file_opts.skip_exist = false;

    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        // These are repository build inputs, not installed runtime payload.
        if matches!(
            entry.file_name().to_str(),
            Some("cache.tar.gz" | "cache.lock.json" | "README.md")
        ) {
            continue;
        }
        let from = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_dir(&from, target, &dir_opts)?;
        } else {
            let to = target.join(entry.file_name());
            copy_file(&from, &to, &file_opts)?;
        }
    }
    Ok(())
}

/// Locations to search for an existing populated gem cache when running tests.
/// Order: env override → per-user bundle → exe-relative install layouts →
/// repo dev fallbacks.
pub fn discover_bundle_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let push = |p: PathBuf, roots: &mut Vec<PathBuf>| {
        if has_gem_layout(&p) && !roots.iter().any(|existing| same_path(existing, &p)) {
            roots.push(p);
        }
    };

    if let Ok(env_path) = std::env::var("REGENT_BUNDLED_GEMS") {
        push(PathBuf::from(env_path), &mut roots);
    }
    if let Some(user) = user_bundle_dir() {
        push(user, &mut roots);
    }
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            for candidate in [
                exe_dir.join(BUNDLED_GEMS_DIRNAME),
                exe_dir
                    .join("..")
                    .join("share")
                    .join("regent")
                    .join(BUNDLED_GEMS_DIRNAME),
                exe_dir.join("..").join(BUNDLED_GEMS_DIRNAME),
            ] {
                push(candidate, &mut roots);
            }
        }
    }
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    push(
        manifest_dir.join("assets").join(BUNDLED_GEMS_DIRNAME),
        &mut roots,
    );
    push(manifest_dir.join("vendor").join("bundle"), &mut roots);
    roots
}

/// Find a source gem cache to copy *from* during bootstrap. This excludes the
/// per-user bundle itself (which is the destination).
fn find_bundled_gems_source() -> Result<Option<PathBuf>> {
    if let Ok(env_path) = std::env::var("REGENT_BUNDLED_GEMS") {
        let candidate = PathBuf::from(env_path);
        if missing_required_gems(&candidate).is_empty() {
            return Ok(Some(candidate));
        }
    }
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            for candidate in [
                exe_dir.join(BUNDLED_GEMS_DIRNAME),
                exe_dir
                    .join("..")
                    .join("share")
                    .join("regent")
                    .join(BUNDLED_GEMS_DIRNAME),
                exe_dir.join("..").join(BUNDLED_GEMS_DIRNAME),
            ] {
                if missing_required_gems(&candidate).is_empty() {
                    return Ok(Some(candidate));
                }
            }
        }
    }
    // The embedded archive is the portable fallback; installation must not
    // depend on the checkout used to compile this binary.
    Ok(None)
}

/// A directory counts as a Regent gem cache only when it follows the Bundler
/// `ruby/<x.y.z>/gems/...` layout. Bare or README-only directories don't
/// count — otherwise we'd "succeed" while copying nothing useful.
fn has_gem_layout(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }
    let ruby_dir = dir.join("ruby");
    let Ok(entries) = std::fs::read_dir(&ruby_dir) else {
        return false;
    };
    for entry in entries.flatten() {
        if entry.path().join("gems").is_dir() {
            return true;
        }
    }
    false
}

fn same_path(a: &Path, b: &Path) -> bool {
    let ac = a.canonicalize().unwrap_or_else(|_| a.to_path_buf());
    let bc = b.canonicalize().unwrap_or_else(|_| b.to_path_buf());
    ac == bc
}

// ---------------------------------------------------------------------------
// Legacy per-module API (deprecated): kept so the older code paths still
// compile. New code should call `ensure_user_bundle` / `discover_bundle_roots`.
// ---------------------------------------------------------------------------

pub fn ensure_bundled_gems(_module_path: &Path) -> Result<Option<PathBuf>> {
    ensure_user_bundle()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn populate_fake_gem_cache(root: &Path) {
        let gems = root.join("ruby/2.6.0/gems");
        for &(name, entrypoint) in REQUIRED_GEMS {
            let file = gems.join(format!("{name}-1.0.0")).join(entrypoint);
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(file, b"# placeholder").unwrap();
        }
        fs::create_dir_all(root.join("ruby/2.6.0/specifications")).unwrap();
    }

    #[test]
    fn copy_contents_into_does_not_nest_source_dir() {
        // Regression test for the v0.5.3 bug where fs_extra's
        // `copy_inside = true` produced `<target>/<source_basename>/ruby/...`
        // instead of `<target>/ruby/...`, which then failed gem verification.
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        populate_fake_gem_cache(src.path());

        copy_contents_into(src.path(), dst.path()).unwrap();

        // Expected (flat) layout:
        assert!(dst
            .path()
            .join("ruby")
            .join("2.6.0")
            .join("gems")
            .join("rspec-1.0.0")
            .is_dir());
        assert!(dst
            .path()
            .join("ruby")
            .join("2.6.0")
            .join("specifications")
            .is_dir());

        // Must NOT have nested the source dir under the target:
        let nested = dst
            .path()
            .join(src.path().file_name().unwrap())
            .join("ruby");
        assert!(
            !nested.exists(),
            "source dir was nested under target at {nested:?}"
        );
    }

    #[test]
    fn copy_contents_into_is_idempotent() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        populate_fake_gem_cache(src.path());

        copy_contents_into(src.path(), dst.path()).unwrap();
        // Second invocation should not fail even though files already exist.
        copy_contents_into(src.path(), dst.path()).unwrap();

        assert!(dst
            .path()
            .join("ruby")
            .join("2.6.0")
            .join("gems")
            .join("rspec-core-1.0.0")
            .is_dir());
    }

    #[test]
    fn copy_excludes_repository_cache_build_inputs() {
        let source = tempdir().unwrap();
        let target = tempdir().unwrap();
        populate_fake_gem_cache(source.path());
        for name in ["cache.tar.gz", "cache.lock.json", "README.md"] {
            fs::write(source.path().join(name), b"build input").unwrap();
        }
        copy_contents_into(source.path(), target.path()).unwrap();
        verify_required_gems(target.path()).unwrap();
        for name in ["cache.tar.gz", "cache.lock.json", "README.md"] {
            assert!(!target.path().join(name).exists());
        }
    }

    #[test]
    fn has_gem_layout_accepts_bundler_tree() {
        let dir = tempdir().unwrap();
        populate_fake_gem_cache(dir.path());
        assert!(has_gem_layout(dir.path()));
    }

    #[test]
    fn has_gem_layout_rejects_readme_only_dir() {
        // The original assets/bundled_gems shipped with only a README. That
        // should not count as a valid gem cache.
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("README.md"), b"placeholder").unwrap();
        assert!(!has_gem_layout(dir.path()));
    }

    #[test]
    fn has_gem_layout_rejects_empty_dir() {
        let dir = tempdir().unwrap();
        assert!(!has_gem_layout(dir.path()));
    }

    #[test]
    fn has_gem_layout_rejects_ruby_dir_without_gems_subdir() {
        let dir = tempdir().unwrap();
        // Looks Bundler-shaped but has no `gems/` underneath.
        fs::create_dir_all(dir.path().join("ruby").join("2.6.0").join("specifications")).unwrap();
        assert!(!has_gem_layout(dir.path()));
    }

    #[test]
    fn embedded_cache_matches_pinned_archive_digest() {
        use sha2::{Digest, Sha256};
        let lock: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/bundled_gems/cache.lock.json"
        )))
        .unwrap();
        let actual: String = Sha256::digest(EMBEDDED_GEM_CACHE)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(
            actual,
            lock["cache_sha256"].as_str().unwrap(),
            "embedded cache is stale: regenerate with `python3 scripts/prepare-gem-cache.py --rebuild`"
        );
    }

    #[test]
    fn embedded_cache_contains_all_required_entrypoints() {
        let staged = tempdir().unwrap();
        unpack_embedded_cache(staged.path()).unwrap();
        verify_required_gems(staged.path()).unwrap();
    }

    #[test]
    fn prefixes_and_missing_entrypoints_do_not_satisfy_required_gems() {
        let cache = tempdir().unwrap();
        populate_fake_gem_cache(cache.path());
        let gems = cache.path().join("ruby/2.6.0/gems");
        fs::remove_dir_all(gems.join("rspec-1.0.0")).unwrap();
        fs::remove_dir_all(gems.join("rspec-puppet-1.0.0")).unwrap();
        fs::remove_file(gems.join("deep_merge-1.0.0/lib/deep_merge.rb")).unwrap();
        assert_eq!(
            missing_required_gems(cache.path()),
            vec!["rspec", "rspec-puppet", "deep_merge"]
        );
    }

    #[test]
    fn valid_installed_cache_does_not_need_original_source() {
        let target = tempdir().unwrap();
        populate_fake_gem_cache(target.path());
        let result = ensure_bundle_at(target.path(), || panic!("source lookup is unnecessary"));
        assert_eq!(result.unwrap(), Some(target.path().to_path_buf()));
    }

    #[test]
    fn incomplete_cache_is_repaired_from_complete_source() {
        let source = tempdir().unwrap();
        let target = tempdir().unwrap();
        populate_fake_gem_cache(source.path());
        populate_fake_gem_cache(target.path());
        let entrypoint = "ruby/2.6.0/gems/rspec-1.0.0/lib/rspec.rb";
        fs::remove_file(target.path().join(entrypoint)).unwrap();
        let existing = "ruby/2.6.0/gems/rspec-core-1.0.0/lib/rspec/core.rb";
        fs::write(target.path().join(existing), b"broken old copy").unwrap();
        ensure_bundle_at(target.path(), || Ok(Some(source.path().to_path_buf()))).unwrap();
        verify_required_gems(target.path()).unwrap();
        assert_eq!(
            fs::read(target.path().join(existing)).unwrap(),
            b"# placeholder"
        );
    }

    #[test]
    fn incomplete_source_is_rejected_before_copying() {
        let source = tempdir().unwrap();
        let target = tempdir().unwrap();
        fs::create_dir_all(source.path().join("ruby/2.6.0/gems/rspec-core-1.0.0/lib")).unwrap();
        let error =
            ensure_bundle_at(target.path(), || Ok(Some(source.path().to_path_buf()))).unwrap_err();
        assert!(error.to_string().contains("rspec"));
        assert!(!target.path().join("ruby").exists());
    }
}
