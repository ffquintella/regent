use anyhow::Result;
use fs_extra::dir::{copy, CopyOptions};
use std::path::{Path, PathBuf};

const BUNDLED_GEMS_DIRNAME: &str = "bundled_gems";

pub fn ensure_bundled_gems(module_path: &Path) -> Result<Option<PathBuf>> {
    let bundled_path = find_bundled_gems_path()?;
    let Some(bundled_path) = bundled_path else {
        return Ok(None);
    };

    let target = module_path.join("vendor").join("bundle");
    std::fs::create_dir_all(&target)?;

    let mut options = CopyOptions::new();
    options.overwrite = false;
    options.copy_inside = true;
    options.skip_exist = true;
    copy(&bundled_path, &target, &options)?;

    Ok(Some(bundled_path))
}

fn find_bundled_gems_path() -> Result<Option<PathBuf>> {
    if let Ok(path) = std::env::var("REGENT_BUNDLED_GEMS") {
        let candidate = PathBuf::from(path);
        if candidate.is_dir() {
            return Ok(Some(candidate));
        }
    }

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let candidates = [
                exe_dir.join(BUNDLED_GEMS_DIRNAME),
                exe_dir.join("..").join("share").join("regent").join(BUNDLED_GEMS_DIRNAME),
                exe_dir.join("..").join(BUNDLED_GEMS_DIRNAME),
            ];
            for candidate in candidates {
                if candidate.is_dir() {
                    return Ok(Some(candidate));
                }
            }
        }
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dev_candidates = [
        manifest_dir.join("assets").join(BUNDLED_GEMS_DIRNAME),
        manifest_dir.join("vendor").join("bundle"),
    ];
    for candidate in dev_candidates {
        if has_gem_layout(&candidate) {
            return Ok(Some(candidate));
        }
    }

    Ok(None)
}

/// A directory counts as a Regent gem cache when it either contains gems
/// directly (Bundler `ruby/x.y.z/gems/...` layout) or wraps that layout.
fn has_gem_layout(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }
    let ruby_dir = dir.join("ruby");
    if ruby_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&ruby_dir) {
            for entry in entries.flatten() {
                if entry.path().join("gems").is_dir() {
                    return true;
                }
            }
        }
    }
    // Allow REGENT_BUNDLED_GEMS to point at a populated-but-empty fallback.
    std::fs::read_dir(dir)
        .map(|mut it| it.next().is_some())
        .unwrap_or(false)
}
