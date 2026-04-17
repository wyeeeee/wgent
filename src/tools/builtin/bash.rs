use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::tools::tool::Tool;

const TIMEOUT_SECS: u64 = 60;
const MAX_OUTPUT_LEN: usize = 10000;

pub struct BashTool;

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "在 bash shell 中执行命令，工作目录为当前会话的工作目录。超时 60 秒。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "要执行的 bash 命令"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, input: Value, working_dir: &Path) -> Result<String> {
        let command = input["command"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 command 参数"))?;

        if command.trim().is_empty() {
            return Err(anyhow!("命令不能为空"));
        }

        let output = tokio::time::timeout(
            Duration::from_secs(TIMEOUT_SECS),
            tokio::process::Command::new("bash")
                .arg("-c")
                .arg(command)
                .current_dir(working_dir)
                .output(),
        )
        .await
        .map_err(|_| anyhow!("命令执行超时（{}秒）", TIMEOUT_SECS))?
        .map_err(|e| anyhow!("启动命令失败: {e}"))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let exit_code = output.status.code().unwrap_or(-1);

        let mut result = String::new();

        if !stdout.is_empty() {
            result.push_str(&truncate(&stdout, MAX_OUTPUT_LEN));
        }
        if !stderr.is_empty() {
            if !result.is_empty() {
                result.push_str("\n");
            }
            result.push_str(&format!("[stderr]\n{}", truncate(&stderr, MAX_OUTPUT_LEN)));
        }
        if exit_code != 0 {
            result.push_str(&format!("\n[退出码: {exit_code}]"));
        }

        Ok(result)
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...\n[输出被截断，共 {} 字符]", &s[..max], s.len())
    }
}
