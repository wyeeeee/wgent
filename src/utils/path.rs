use std::path::{Path, PathBuf};

use anyhow::Result;

/// Resolve a path: absolute paths are used as-is, relative paths are joined to working_dir
pub fn resolve_path(working_dir: &Path, path: &str) -> Result<PathBuf> {
    let p = PathBuf::from(path);
    if p.is_absolute() {
        Ok(p)
    } else {
        Ok(working_dir.join(p))
    }
}
