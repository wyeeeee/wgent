use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::config::Config;
use crate::tools::tool::Tool;

pub struct BashTool {
    config: Config,
}

impl BashTool {
    pub fn new(config: Config) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "在系统 shell 中执行命令（Windows: pwsh, Unix: bash），工作目录为当前会话的工作目录。有超时限制。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "要执行的 shell 命令"
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

        let timeout_secs = self.config.get().command_timeout;

        let output = tokio::time::timeout(
            Duration::from_secs(timeout_secs),
            shell_command(command, working_dir),
        )
        .await
        .map_err(|_| anyhow!("命令执行超时（{}秒）", timeout_secs))?
        .map_err(|e| anyhow!("启动命令失败: {e}"))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let exit_code = output.status.code().unwrap_or(-1);

        let mut result = String::new();

        if !stdout.is_empty() {
            result.push_str(&stdout);
        }
        if !stderr.is_empty() {
            if !result.is_empty() {
                result.push_str("\n");
            }
            result.push_str(&format!("[stderr]\n{stderr}"));
        }
        if exit_code != 0 {
            result.push_str(&format!("\n[退出码: {exit_code}]"));
        }

        Ok(result)
    }
}

/// 按平台选择 shell 执行命令
async fn shell_command(command: &str, working_dir: &Path) -> std::io::Result<std::process::Output> {
    if cfg!(windows) {
        tokio::process::Command::new("pwsh")
            .arg("-NoProfile")
            .arg("-Command")
            .arg(command)
            .current_dir(working_dir)
            .output()
            .await
    } else {
        tokio::process::Command::new("bash")
            .arg("-c")
            .arg(command)
            .current_dir(working_dir)
            .output()
            .await
    }
}
