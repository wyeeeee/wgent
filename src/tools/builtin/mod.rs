pub mod bash;
pub mod edit;
pub mod read;
pub mod write;

pub use bash::BashTool;
pub use edit::EditTool;
pub use read::ReadTool;
pub use write::WriteTool;

use anyhow::Result;
use std::path::{Path, PathBuf};

/// 相对路径解析：绝对路径直接用，相对路径拼接到 working_dir
fn resolve_path(working_dir: &Path, path: &str) -> Result<PathBuf> {
    let p = PathBuf::from(path);
    if p.is_absolute() {
        Ok(p)
    } else {
        Ok(working_dir.join(p))
    }
}
