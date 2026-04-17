use std::path::Path;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::tools::tool::Tool;
use super::resolve_path;

pub struct ReadTool;

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str {
        "read"
    }

    fn description(&self) -> &str {
        "读取文件内容，返回带行号的文本。用于查看代码、配置等文件。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "文件路径（相对于工作目录或绝对路径）"
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, input: Value, working_dir: &Path) -> Result<String> {
        let path_str = input["path"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 path 参数"))?;
        let path = resolve_path(working_dir, path_str)?;

        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| anyhow!("读取文件失败 {}: {e}", path.display()))?;

        let total_lines = content.lines().count();
        let numbered: String = content
            .lines()
            .enumerate()
            .map(|(i, line)| format!("{:>4} | {}", i + 1, line))
            .collect::<Vec<_>>()
            .join("\n");

        Ok(format!("文件: {} (共 {} 行)\n{}", path.display(), total_lines, numbered))
    }
}
