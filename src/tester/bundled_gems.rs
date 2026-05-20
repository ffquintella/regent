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
