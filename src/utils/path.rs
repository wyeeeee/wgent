use std::path::{Path, PathBuf};

use anyhow::Result;

/// 相对路径解析：绝对路径直接用，相对路径拼接到 working_dir
pub fn resolve_path(working_dir: &Path, path: &str) -> Result<PathBuf> {
    let p = PathBuf::from(path);
    if p.is_absolute() {
        Ok(p)
    } else {
        Ok(working_dir.join(p))
    }
}
