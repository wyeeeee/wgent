use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::tools::tool::{Tool, ToolContext};
use crate::utils::resolve_path;

pub struct MultiEditTool;

#[async_trait]
impl Tool for MultiEditTool {
    fn name(&self) -> &str {
        "MultiEdit"
    }

    fn description(&self) -> &str {
        "Apply multiple text replacements to one file in a single operation. Each edit replaces old_string with new_string (empty string to delete). Edits apply sequentially — later edits see earlier results. All must succeed or nothing is written."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path (relative to working directory or absolute)"
                },
                "edits": {
                    "type": "array",
                    "description": "List of edits to apply sequentially",
                    "items": {
                        "type": "object",
                        "properties": {
                            "old_string": {
                                "type": "string",
                                "description": "Exact text to find — must match exactly one location after prior edits"
                            },
                            "new_string": {
                                "type": "string",
                                "description": "Replacement text. Pass empty string to delete the matched text."
                            }
                        },
                        "required": ["old_string", "new_string"]
                    }
                }
            },
            "required": ["path", "edits"]
        })
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String> {
        let path_str = input["path"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing 'path' parameter"))?;
        let path = resolve_path(&ctx.working_dir, path_str)?;

        let edits_arr = input
            .get("edits")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow!("Missing or invalid 'edits' parameter"))?;

        if edits_arr.is_empty() {
            return Err(anyhow!("'edits' array is empty"));
        }

        let mut edits = Vec::with_capacity(edits_arr.len());
        for (i, item) in edits_arr.iter().enumerate() {
            let old = item
                .get("old_string")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("edits[{i}]: missing 'old_string'"))?;
            let new_str = item.get("new_string").and_then(|v| v.as_str()).unwrap_or("");
            edits.push((old, new_str));
        }

        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| anyhow!("Failed to read file {}: {e}", path.display()))?;

        let mut result = content;
        let mut summary_parts = Vec::with_capacity(edits.len());

        for (i, (old, new_str)) in edits.iter().enumerate() {
            let count = result.matches(*old).count();
            match count {
                0 => {
                    return Err(anyhow!(
                        "Edit {}: old_string not found in file. Ensure the text matches exactly.",
                        i + 1
                    ));
                }
                1 => {
                    result = result.replacen(old, new_str, 1);
                    let old_lines = old.lines().count();
                    let new_lines = new_str.lines().count();
                    let action = if new_str.is_empty() {
                        format!("deleted {} line(s)", old_lines)
                    } else {
                        format!("replaced {} line(s) → {} line(s)", old_lines, new_lines)
                    };
                    summary_parts.push(format!("  [{}] {}", i + 1, action));
                }
                _ => {
                    return Err(anyhow!(
                        "Edit {}: old_string matches {} locations. Provide more context to make it unique.",
                        i + 1,
                        count
                    ));
                }
            }
        }

        tokio::fs::write(&path, &result)
            .await
            .map_err(|e| anyhow!("Failed to write file {}: {e}", path.display()))?;

        Ok(format!(
            "Edited: {} ({} changes)\n{}",
            path.display(),
            summary_parts.len(),
            summary_parts.join("\n")
        ))
    }
}