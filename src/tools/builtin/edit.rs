use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::tools::tool::{Tool, ToolContext};
use crate::utils::resolve_path;

pub struct EditTool;

#[async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str {
        "Edit"
    }

    fn description(&self) -> &str {
        "Edit a file by replacing exact text matches. Provide 'old_string' (must match exactly one location) and 'new_string' (replacement text). Read the file first to get exact text, then copy the portion to replace."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path (relative to working directory or absolute)"
                },
                "old_string": {
                    "type": "string",
                    "description": "Exact text to find — must match exactly one location in the file"
                },
                "new_string": {
                    "type": "string",
                    "description": "Replacement text. Empty string deletes the matched text."
                }
            },
            "required": ["path", "old_string", "new_string"]
        })
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String> {
        let path_str = input["path"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing 'path' parameter"))?;
        let path = resolve_path(&ctx.working_dir, path_str)?;

        let old = input["old_string"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing 'old_string' parameter"))?;
        let new_str = input["new_string"].as_str().unwrap_or("");

        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| anyhow!("Failed to read file {}: {e}", path.display()))?;

        let count = content.matches(old).count();
        match count {
            0 => Err(anyhow!(
                "old_string not found in file. Ensure the text matches exactly, including whitespace and indentation."
            )),
            1 => {
                let result = content.replacen(old, new_str, 1);
                tokio::fs::write(&path, &result)
                    .await
                    .map_err(|e| anyhow!("Failed to write file {}: {e}", path.display()))?;

                let old_lines = old.lines().count();
                let new_lines = new_str.lines().count();
                let action = if new_str.is_empty() {
                    format!("deleted {} line(s)", old_lines)
                } else {
                    format!("replaced {} line(s) → {} line(s)", old_lines, new_lines)
                };
                Ok(format!("Edited: {} ({})", path.display(), action))
            }
            _ => Err(anyhow!(
                "old_string matches {} locations. Provide more surrounding context to make it unique.",
                count
            )),
        }
    }
}