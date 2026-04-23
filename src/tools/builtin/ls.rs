use std::io::{BufRead, BufReader};
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
        "Ls"
    }

    fn description(&self) -> &str {
        "List directory contents with line counts for files. Always start with depth 1 to get an overview."
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

        let display_path = path.display().to_string();
        let output = tokio::task::spawn_blocking(move || {
            let mut output = String::new();
            list_dir(&path, depth, 0, &mut output)?;
            Ok::<String, anyhow::Error>(output)
        })
        .await
        .map_err(|e| anyhow!("Ls task failed: {e}"))??;

        if output.is_empty() {
            return Ok(format!("{} (empty directory)", display_path));
        }

        Ok(format!("{}\n{}", display_path, output))
    }
}

/// Try to count lines for a file. Returns None if the file appears to be binary.
/// Uses BufReader to avoid loading the entire file into memory.
fn try_count_lines(path: &Path) -> Option<usize> {
    let file = std::fs::File::open(path).ok()?;
    let mut reader = BufReader::new(file);
    let mut count = 0;
    let mut buf = Vec::new();

    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf) {
            Ok(0) => break,
            Ok(_) => {
                // Binary check on first line only
                if count == 0 && buf.contains(&0) {
                    return None;
                }
                count += 1;
            }
            Err(_) => break,
        }
    }

    Some(if count == 0 { 1 } else { count })
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
            output.push_str(&format!("{prefix}{name}/\n"));
            if current_depth + 1 < max_depth {
                list_dir(&path, max_depth, current_depth + 1, output)?;
            }
        } else if let Some(lines) = try_count_lines(&path) {
            output.push_str(&format!("{prefix}{name} ({lines} lines)\n"));
        }
    }

    Ok(())
}
