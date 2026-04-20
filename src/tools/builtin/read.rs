use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::tools::tool::{Tool, ToolContext};
use crate::utils::resolve_path;

pub struct ReadTool;

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str {
        "read"
    }

    fn description(&self) -> &str {
        "读取文件内容，返回带行号的文本。可指定行范围，不指定则返回全部内容。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "文件路径（相对于工作目录或绝对路径）"
                },
                "start_line": {
                    "type": "integer",
                    "description": "起始行号（1-indexed，inclusive，可选，默认 1）"
                },
                "end_line": {
                    "type": "integer",
                    "description": "结束行号（1-indexed，inclusive，可选，默认文件末尾）"
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String> {
        let path_str = input["path"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 path 参数"))?;
        let path = resolve_path(&ctx.working_dir, path_str)?;

        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| anyhow!("读取文件失败 {}: {e}", path.display()))?;

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
            return Err(anyhow!("start_line {} 超出范围（文件共 {total_lines} 行）", start));
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
            format!("共 {} 行", total_lines)
        } else {
            format!("第 {}-{} 行 / 共 {} 行", start, end, total_lines)
        };

        Ok(format!("文件: {} ({})\n{}", path.display(), range_info, numbered))
    }
}
