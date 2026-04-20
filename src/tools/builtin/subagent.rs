use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::core::Agent;
use crate::tools::tool::{Tool, ToolContext};
use crate::transport::AgentEvent;

pub struct SubAgentTool {
    config_dir: PathBuf,
    working_dir: PathBuf,
}

impl SubAgentTool {
    pub fn new(config_dir: PathBuf, working_dir: PathBuf) -> Self {
        Self { config_dir, working_dir }
    }
}

#[async_trait]
impl Tool for SubAgentTool {
    fn name(&self) -> &str {
        "subagent"
    }

    fn description(&self) -> &str {
        "生成子代理执行子任务。子代理可独立使用工具，完成后返回结果文本。不可递归调用。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task": {
                    "type": "string",
                    "description": "子代理需要完成的任务描述"
                }
            },
            "required": ["task"]
        })
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String> {
        let task = input["task"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 task 参数"))?;

        if task.trim().is_empty() {
            return Err(anyhow!("task 不能为空"));
        }

        let agent = Arc::new(Agent::new_sub(&self.config_dir, &self.working_dir)?);
        let (_, mut rx) = agent.chat(None, task).await?;

        let mut last_text = None;
        while let Some(event) = rx.recv().await {
            if let Some(tx) = &ctx.events {
                let _ = tx.send(event.clone()).await;
            }
            if let AgentEvent::TextComplete(text) = event {
                last_text = Some(text);
            }
        }

        Ok(last_text.unwrap_or_else(|| "(子代理未返回文本结果)".into()))
    }
}
