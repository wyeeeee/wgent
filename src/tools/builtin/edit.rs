use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::tools::tool::{Tool, ToolContext};
use crate::utils::resolve_path;

pub struct EditTool;

#[async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str {
        "edit"
    }

    fn description(&self) -> &str {
        "Edit a file by replacing a specified line range. Use read first to view line numbers, then use this tool to specify the range to replace."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path"
                },
                "start_line": {
                    "type": "integer",
                    "description": "Start line (1-indexed, inclusive)"
                },
                "end_line": {
                    "type": "integer",
                    "description": "End line (1-indexed, inclusive)"
                },
                "old_content": {
                    "type": "string",
                    "description": "Expected original content (optional, used to verify the correct location)"
                },
                "new_content": {
                    "type": "string",
                    "description": "New replacement content (empty string to delete the line range)"
                }
            },
            "required": ["path", "start_line", "end_line", "new_content"]
        })
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String> {
        let path_str = input["path"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing 'path' parameter"))?;
        let start_line = input["start_line"]
            .as_u64()
            .ok_or_else(|| anyhow!("Missing 'start_line' parameter"))? as usize;
        let end_line = input["end_line"]
            .as_u64()
            .ok_or_else(|| anyhow!("Missing 'end_line' parameter"))? as usize;
        let new_content = input["new_content"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let old_content = input.get("old_content").and_then(|v| v.as_str());

        let path = resolve_path(&ctx.working_dir, path_str)?;
        let raw = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| anyhow!("Failed to read file {}: {e}", path.display()))?;

        let mut lines: Vec<String> = raw.lines().map(|l| l.to_string()).collect();
        let trailing_newline = raw.ends_with('\n');
        let total = lines.len();

        if start_line == 0 || start_line > total {
            return Err(anyhow!("start_line {} out of range (file has {total} lines)", start_line));
        }
        if end_line == 0 || end_line > total {
            return Err(anyhow!("end_line {} out of range (file has {total} lines)", end_line));
        }
        if start_line > end_line {
            return Err(anyhow!("start_line({start_line}) > end_line({end_line})"));
        }

        if let Some(expected) = old_content {
            let actual: String = lines[start_line - 1..end_line].join("\n");
            if actual.trim() != expected.trim() {
                return Err(anyhow!(
                    "Content mismatch (lines {}-{}):\n--- expected ---\n{}\n--- actual ---\n{}",
                    start_line, end_line, expected, actual
                ));
            }
        }

        let old_lines: String = lines[start_line - 1..end_line].join("\n");

        let replacement: Vec<String> = if new_content.is_empty() {
            Vec::new()
        } else {
            new_content.lines().map(|l| l.to_string()).collect()
        };
        lines.splice(start_line - 1..end_line, replacement);

        let mut result = lines.join("\n");
        if trailing_newline {
            result.push('\n');
        }
        tokio::fs::write(&path, &result)
            .await
            .map_err(|e| anyhow!("Failed to write file {}: {e}", path.display()))?;

        let new_line_count = if new_content.is_empty() { 0 } else { new_content.lines().count() };
        Ok(format!(
            "Edited: {} (lines {}-{} → {} lines)\n--- old ---\n{}\n--- new ---\n{}",
            path.display(),
            start_line,
            end_line,
            new_line_count,
            old_lines,
            new_content,
        ))
    }
}
