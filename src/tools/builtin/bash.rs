use std::time::Duration;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::config::Config;
use crate::tools::tool::{Tool, ToolContext};

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
        "Execute a command in the system shell (Windows: pwsh, Unix: bash). Uses the session working directory. Subject to timeout."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String> {
        let command = input["command"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing 'command' parameter"))?;

        if command.trim().is_empty() {
            return Err(anyhow!("Command cannot be empty"));
        }

        let timeout_secs = self.config.get().command_timeout;

        let output = tokio::time::timeout(
            Duration::from_secs(timeout_secs),
            shell_command(command, &ctx.working_dir),
        )
        .await
        .map_err(|_| anyhow!("Command timed out ({}s)", timeout_secs))?
        .map_err(|e| anyhow!("Failed to start command: {e}"))?;

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
            result.push_str(&format!("\n[exit code: {exit_code}]"));
        }

        Ok(result)
    }
}

async fn shell_command(command: &str, working_dir: &std::path::Path) -> std::io::Result<std::process::Output> {
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
