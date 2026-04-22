use std::ffi::OsStr;
use std::path::Path;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::tools::tool::{Tool, ToolContext};
use crate::utils::resolve_path;

const MAX_DEPTH: u8 = 10;

pub struct LsTool;

#[async_trait]
impl Tool for LsTool {
    fn name(&self) -> &str {
        "ls"
    }

    fn description(&self) -> &str {
        "List directory contents with line counts for files and aggregate info for subdirectories."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Directory path (relative to working directory or absolute)"
                },
                "depth": {
                    "type": "integer",
                    "description": "Recursion depth (1 = immediate children only, default 1, max 10)"
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String> {
        let path_str = input["path"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing 'path' parameter"))?;
        let path = resolve_path(&ctx.working_dir, path_str)?;

        if !path.is_dir() {
            return Err(anyhow!("{} is not a directory", path.display()));
        }

        let depth = input
            .get("depth")
            .and_then(|v| v.as_u64())
            .unwrap_or(1)
            .min(MAX_DEPTH as u64) as u8;

        let mut output = String::new();
        list_dir(&path, depth, 0, &mut output)?;

        if output.is_empty() {
            return Ok(format!("{} (empty directory)", path.display()));
        }

        Ok(format!("{}\n{}", path.display(), output))
    }
}

struct Aggregate {
    file_count: usize,
    dir_count: usize,
    total_lines: usize,
}

/// Only scan one level deep — no recursive descent into subdirectories.
fn shallow_aggregate(dir: &Path) -> Aggregate {
    let mut agg = Aggregate {
        file_count: 0,
        dir_count: 0,
        total_lines: 0,
    };

    let Ok(entries) = std::fs::read_dir(dir) else {
        return agg;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            agg.dir_count += 1;
        } else {
            agg.file_count += 1;
            agg.total_lines += count_lines(&path);
        }
    }

    agg
}

fn count_lines(path: &Path) -> usize {
    if is_binary(path) {
        return 0;
    }
    std::fs::read_to_string(path).map(|s| s.lines().count()).unwrap_or(0)
}

fn is_binary(path: &Path) -> bool {
    let ext = path.extension().and_then(OsStr::to_str).unwrap_or("");
    matches!(
        ext,
        "exe" | "dll" | "so" | "dylib" | "png" | "jpg" | "jpeg" | "gif" | "bmp" | "ico"
            | "webp" | "svg" | "woff" | "woff2" | "ttf" | "otf" | "eot" | "zip" | "tar"
            | "gz" | "bz2" | "xz" | "7z" | "rar" | "pdf" | "doc" | "docx" | "xls" | "xlsx"
            | "ppt" | "pptx" | "mp3" | "mp4" | "avi" | "mov" | "wav" | "flac" | "ogg"
            | "webm" | "mkv" | "class" | "o" | "obj" | "pyc" | "wasm"
    )
}

fn list_dir(dir: &Path, max_depth: u8, current_depth: u8, output: &mut String) -> Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| anyhow!("Failed to read directory {}: {e}", dir.display()))?
        .filter_map(|e| e.ok())
        .collect();

    entries.sort_by_key(|e| e.file_name());

    let last_idx = entries.len().saturating_sub(1);

    for (i, entry) in entries.into_iter().enumerate() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_last = i == last_idx;

        let prefix = if current_depth == 0 {
            if is_last { "└── " } else { "├── " }.to_string()
        } else {
            let mut s = String::new();
            for _ in 0..current_depth {
                s.push_str("│   ");
            }
            s.push_str(if is_last { "└── " } else { "├── " });
            s
        };

        if path.is_dir() {
            let expanded = current_depth + 1 < max_depth;
            let agg = shallow_aggregate(&path);

            let label = if expanded {
                format!("{}/ ({} files, {} dirs, {} lines)", name, agg.file_count, agg.dir_count, agg.total_lines)
            } else {
                format!("{}/ ({} files, {} dirs, {} lines) [preview]", name, agg.file_count, agg.dir_count, agg.total_lines)
            };

            output.push_str(&format!("{prefix}{label}\n"));

            if expanded {
                list_dir(&path, max_depth, current_depth + 1, output)?;
            }
        } else if is_binary(&path) {
            output.push_str(&format!("{prefix}{name} (binary)\n"));
        } else {
            let lines = count_lines(&path);
            output.push_str(&format!("{prefix}{name} ({lines} lines)\n"));
        }
    }

    Ok(())
}
