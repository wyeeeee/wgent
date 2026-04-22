use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::tools::tool::{Tool, ToolContext};
use crate::utils::resolve_path;

pub struct WriteTool;

#[async_trait]
impl Tool for WriteTool {
    fn name(&self) -> &str {
        "write"
    }

    fn description(&self) -> &str {
        "Create or overwrite a file. Automatically creates parent directories if they don't exist."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path (relative to working directory or absolute)"
                },
                "content": {
                    "type": "string",
                    "description": "The complete file content to write"
                }
            },
            "required": ["path", "content"]
        })
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String> {
        let path_str = input["path"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing 'path' parameter"))?;
        let content = input["content"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing 'content' parameter"))?;
        let path = resolve_path(&ctx.working_dir, path_str)?;

        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::write(&path, content)
            .await
            .map_err(|e| anyhow!("Failed to write file {}: {e}", path.display()))?;

        let lines = content.lines().count();
        Ok(format!("Written: {} ({} lines)", path.display(), lines))
    }
}
