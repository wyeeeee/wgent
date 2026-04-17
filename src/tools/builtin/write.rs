use std::path::Path;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::tools::tool::Tool;
use super::resolve_path;

pub struct WriteTool;

#[async_trait]
impl Tool for WriteTool {
    fn name(&self) -> &str {
        "write"
    }

    fn description(&self) -> &str {
        "创建或覆盖文件。会自动创建不存在的父目录。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "文件路径（相对于工作目录或绝对路径）"
                },
                "content": {
                    "type": "string",
                    "description": "要写入的完整文件内容"
                }
            },
            "required": ["path", "content"]
        })
    }

    async fn execute(&self, input: Value, working_dir: &Path) -> Result<String> {
        let path_str = input["path"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 path 参数"))?;
        let content = input["content"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 content 参数"))?;
        let path = resolve_path(working_dir, path_str)?;

        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::write(&path, content)
            .await
            .map_err(|e| anyhow!("写入文件失败 {}: {e}", path.display()))?;

        let lines = content.lines().count();
        Ok(format!("已写入: {} ({} 行)", path.display(), lines))
    }
}
