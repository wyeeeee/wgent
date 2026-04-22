use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::tools::tool::{Tool, ToolContext};
use crate::utils::resolve_path;

pub struct ReadTool;

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str {
        "Read"
    }

    fn description(&self) -> &str {
        "Read file contents and return line-numbered text. Optionally specify a line range; returns the entire file if omitted."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path (relative to working directory or absolute)"
                },
                "start_line": {
                    "type": "integer",
                    "description": "Start line (1-indexed, inclusive, optional, default 1)"
                },
                "end_line": {
                    "type": "integer",
                    "description": "End line (1-indexed, inclusive, optional, default end of file)"
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

        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| anyhow!("Failed to read file {}: {e}", path.display()))?;

        let all_lines: Vec<&str> = content.lines().collect();
        let total_lines = all_lines.len();

        let start = input
            .get("start_line")
            .and_then(|v| v.as_u64())
            .map(|v| (v as usize).max(1))
            .unwrap_or(1);

        let end = input
            .get("end_line")
            .and_then(|v| v.as_u64())
            .map(|v| (v as usize).min(total_lines))
            .unwrap_or(total_lines);

        if start > total_lines {
            return Err(anyhow!("start_line {} out of range (file has {total_lines} lines)", start));
        }
        if start > end {
            return Err(anyhow!("start_line({start}) > end_line({end})"));
        }

        let numbered: String = all_lines[start - 1..end]
            .iter()
            .enumerate()
            .map(|(i, line)| format!("{:>4} | {}", start + i, line))
            .collect::<Vec<_>>()
            .join("\n");

        let range_info = if start == 1 && end == total_lines {
            format!("{} lines total", total_lines)
        } else {
            format!("lines {}-{} / {} total", start, end, total_lines)
        };

        Ok(format!("File: {} ({})\n{}", path.display(), range_info, numbered))
    }
}
