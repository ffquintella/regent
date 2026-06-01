use anyhow::{Context, Result};
use fs_extra::dir::{copy as copy_dir, CopyOptions};
use fs_extra::file::{copy as copy_file, CopyOptions as FileCopyOptions};
use std::path::{Path, PathBuf};

const BUNDLED_GEMS_DIRNAME: &str = "bundled_gems";

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
/// or `None` if no source cache could be located.
pub fn ensure_user_bundle() -> Result<Option<PathBuf>> {
    let Some(target) = user_bundle_dir() else {
        return Ok(None);
    };
    let Some(source) = find_bundled_gems_source()? else {
        return Ok(None);
    };
    if same_path(&source, &target) {
        return Ok(Some(source));
    }
    std::fs::create_dir_all(&target).with_context(|| {
        format!("creating Regent user bundle dir {}", target.display())
    })?;
    copy_contents_into(&source, &target)
        .with_context(|| format!("copying gem cache {} -> {}", source.display(), target.display()))?;
    Ok(Some(source))
}

/// Copy each immediate child of `source` into `target`. fs_extra's
/// `copy_inside = true` does NOT do this — it nests the source dir under the
/// target. Doing it ourselves keeps the layout predictable.
fn copy_contents_into(source: &Path, target: &Path) -> Result<()> {
    let mut dir_opts = CopyOptions::new();
    dir_opts.overwrite = false;
    dir_opts.skip_exist = true;
    dir_opts.copy_inside = false;

    let mut file_opts = FileCopyOptions::new();
    file_opts.overwrite = false;
    file_opts.skip_exist = true;

    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
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
                exe_dir.join("..").join("share").join("regent").join(BUNDLED_GEMS_DIRNAME),
                exe_dir.join("..").join(BUNDLED_GEMS_DIRNAME),
            ] {
                push(candidate, &mut roots);
            }
        }
    }
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    push(manifest_dir.join("assets").join(BUNDLED_GEMS_DIRNAME), &mut roots);
    push(manifest_dir.join("vendor").join("bundle"), &mut roots);
    roots
}

/// Find a source gem cache to copy *from* during bootstrap. This excludes the
/// per-user bundle itself (which is the destination).
fn find_bundled_gems_source() -> Result<Option<PathBuf>> {
    if let Ok(env_path) = std::env::var("REGENT_BUNDLED_GEMS") {
        let candidate = PathBuf::from(env_path);
        if has_gem_layout(&candidate) {
            return Ok(Some(candidate));
        }
    }
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            for candidate in [
                exe_dir.join(BUNDLED_GEMS_DIRNAME),
                exe_dir.join("..").join("share").join("regent").join(BUNDLED_GEMS_DIRNAME),
                exe_dir.join("..").join(BUNDLED_GEMS_DIRNAME),
            ] {
                if has_gem_layout(&candidate) {
                    return Ok(Some(candidate));
                }
            }
        }
    }
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for candidate in [
        manifest_dir.join("assets").join(BUNDLED_GEMS_DIRNAME),
        manifest_dir.join("vendor").join("bundle"),
    ] {
        if has_gem_layout(&candidate) {
            return Ok(Some(candidate));
        }
    }
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

    /// Build a minimal but realistic Bundler-style gem cache under `root`:
    ///
    /// ```text
    /// root/
    ///   ruby/2.6.0/gems/rspec-3.13.2/lib/rspec.rb
    ///   ruby/2.6.0/gems/rspec-core-3.13.6/lib/rspec/core.rb
    ///   ruby/2.6.0/specifications/rspec-3.13.2.gemspec
    /// ```
    fn populate_fake_gem_cache(root: &Path) {
        let gems = root.join("ruby").join("2.6.0").join("gems");
        for gem in ["rspec-3.13.2", "rspec-core-3.13.6"] {
            let lib = gems.join(gem).join("lib");
            fs::create_dir_all(&lib).unwrap();
            fs::write(lib.join("placeholder.rb"), b"# placeholder").unwrap();
        }
        let specs = root.join("ruby").join("2.6.0").join("specifications");
        fs::create_dir_all(&specs).unwrap();
        fs::write(specs.join("rspec-3.13.2.gemspec"), b"# spec").unwrap();
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
        assert!(dst.path().join("ruby").join("2.6.0").join("gems").join("rspec-3.13.2").is_dir());
        assert!(dst.path().join("ruby").join("2.6.0").join("specifications").is_dir());

        // Must NOT have nested the source dir under the target:
        let nested = dst.path().join(src.path().file_name().unwrap()).join("ruby");
        assert!(!nested.exists(), "source dir was nested under target at {nested:?}");
    }

    #[test]
    fn copy_contents_into_is_idempotent() {
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        populate_fake_gem_cache(src.path());

        copy_contents_into(src.path(), dst.path()).unwrap();
        // Second invocation should not fail even though files already exist.
        copy_contents_into(src.path(), dst.path()).unwrap();

        assert!(dst.path().join("ruby").join("2.6.0").join("gems").join("rspec-core-3.13.6").is_dir());
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

    /// The Regent repo's own `vendor/bundle` is the canonical shipped cache.
    /// If anything in REQUIRED_GEMS is missing from it, `regent bootstrap`
    /// will fail for end users — fail the build instead so we notice first.
    #[test]
    fn shipped_vendor_bundle_contains_all_required_gems() {
        let bundle = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("vendor")
            .join("bundle");
        if !has_gem_layout(&bundle) {
            // No shipped cache in the dev tree — skip rather than fail; CI
            // environments may build without it.
            return;
        }
        let ruby_root = bundle.join("ruby");
        let required = [
            "rspec",
            "rspec-core",
            "rspec-expectations",
            "rspec-support",
            "rspec-puppet",
            "rspec-puppet-facts",
            "facterdb",
            "deep_merge",
        ];
        let mut found: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for entry in fs::read_dir(&ruby_root).unwrap().flatten() {
            let Ok(gems) = fs::read_dir(entry.path().join("gems")) else { continue };
            for gem in gems.flatten() {
                let Some(name) = gem.file_name().to_str().map(str::to_owned) else { continue };
                for req in &required {
                    if name.starts_with(&format!("{req}-")) {
                        found.insert(req);
                    }
                }
            }
        }
        let missing: Vec<&&str> = required.iter().filter(|r| !found.contains(*r)).collect();
        assert!(
            missing.is_empty(),
            "shipped vendor/bundle is missing required gem(s): {missing:?}"
        );
    }

    #[test]
    fn verify_required_gems_layout_matches_copy_output() {
        // The verify step that runs during `regent bootstrap` reads
        // `<bundle>/ruby/*/gems/<name>-<version>`. Ensure the layout produced
        // by copy_contents_into is exactly what that check expects.
        let src = tempdir().unwrap();
        let dst = tempdir().unwrap();
        populate_fake_gem_cache(src.path());

        copy_contents_into(src.path(), dst.path()).unwrap();

        let ruby_root = dst.path().join("ruby");
        let mut found_rspec = false;
        for entry in fs::read_dir(&ruby_root).unwrap().flatten() {
            for gem in fs::read_dir(entry.path().join("gems")).unwrap().flatten() {
                if gem.file_name().to_string_lossy().starts_with("rspec-") {
                    found_rspec = true;
                }
            }
        }
        assert!(found_rspec, "verify_required_gems would have missed rspec in {ruby_root:?}");
    }
}
