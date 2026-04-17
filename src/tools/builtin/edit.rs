use std::path::Path;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::tools::tool::Tool;
use super::resolve_path;

pub struct EditTool;

#[async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str {
        "edit"
    }

    fn description(&self) -> &str {
        "精确编辑文件：替换指定行范围的内容。先用 read 查看行号，再用本工具指定行范围进行替换。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "文件路径"
                },
                "start_line": {
                    "type": "integer",
                    "description": "起始行号（1-indexed，inclusive）"
                },
                "end_line": {
                    "type": "integer",
                    "description": "结束行号（1-indexed，inclusive）"
                },
                "old_content": {
                    "type": "string",
                    "description": "期望被替换的原内容（可选，用于校验防止改错位置）"
                },
                "new_content": {
                    "type": "string",
                    "description": "替换后的新内容（空字符串表示删除该行范围）"
                }
            },
            "required": ["path", "start_line", "end_line", "new_content"]
        })
    }

    async fn execute(&self, input: Value, working_dir: &Path) -> Result<String> {
        let path_str = input["path"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 path 参数"))?;
        let start_line = input["start_line"]
            .as_u64()
            .ok_or_else(|| anyhow!("缺少 start_line 参数"))? as usize;
        let end_line = input["end_line"]
            .as_u64()
            .ok_or_else(|| anyhow!("缺少 end_line 参数"))? as usize;
        let new_content = input["new_content"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let old_content = input.get("old_content").and_then(|v| v.as_str());

        let path = resolve_path(working_dir, path_str)?;
        let raw = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| anyhow!("读取文件失败 {}: {e}", path.display()))?;

        let mut lines: Vec<String> = raw.lines().map(|l| l.to_string()).collect();
        let total = lines.len();

        if start_line == 0 || start_line > total {
            return Err(anyhow!("start_line {} 超出范围（文件共 {total} 行）", start_line));
        }
        if end_line == 0 || end_line > total {
            return Err(anyhow!("end_line {} 超出范围（文件共 {total} 行）", end_line));
        }
        if start_line > end_line {
            return Err(anyhow!("start_line({start_line}) > end_line({end_line})"));
        }

        // 内容校验
        if let Some(expected) = old_content {
            let actual: String = lines[start_line - 1..end_line].join("\n");
            if actual.trim() != expected.trim() {
                return Err(anyhow!(
                    "内容校验失败（第 {}-{} 行）:\n--- 期望 ---\n{}\n--- 实际 ---\n{}",
                    start_line, end_line, expected, actual
                ));
            }
        }

        // 记录旧内容用于 diff 展示
        let old_lines: String = lines[start_line - 1..end_line].join("\n");

        // 执行替换
        let replacement: Vec<String> = if new_content.is_empty() {
            Vec::new()
        } else {
            new_content.lines().map(|l| l.to_string()).collect()
        };
        lines.splice(start_line - 1..end_line, replacement);

        let result = lines.join("\n");
        tokio::fs::write(&path, &result)
            .await
            .map_err(|e| anyhow!("写入文件失败 {}: {e}", path.display()))?;

        let new_line_count = if new_content.is_empty() { 0 } else { new_content.lines().count() };
        Ok(format!(
            "已编辑: {} (第 {}-{} 行 → {} 行)\n--- 旧 ---\n{}\n--- 新 ---\n{}",
            path.display(),
            start_line,
            end_line,
            new_line_count,
            if old_lines.len() > 500 { format!("{}...(截断)", &old_lines[..500]) } else { old_lines },
            if new_content.len() > 500 { format!("{}...(截断)", &new_content[..500]) } else { new_content },
        ))
    }
}
