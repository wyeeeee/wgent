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
        "SubAgent"
    }

    fn description(&self) -> &str {
        "Spawn a sub-agent to execute a sub-task. The sub-agent can use tools independently and returns the result text. Cannot be called recursively."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task": {
                    "type": "string",
                    "description": "Task description for the sub-agent to complete"
                }
            },
            "required": ["task"]
        })
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String> {
        let task = input["task"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing 'task' parameter"))?;

        if task.trim().is_empty() {
            return Err(anyhow!("Task cannot be empty"));
        }

        let agent = Arc::new(Agent::new_sub(&self.config_dir, &self.working_dir)?);
        let (_, mut rx) = agent.chat(None, task).await?;

        let mut last_text = None;
        while let Some(event) = rx.recv().await {
            if let Some(tx) = &ctx.events
                && tx.send(event.clone()).await.is_err()
            {
                break;
            }
            if let AgentEvent::TextDelta(text) = event {
                last_text = Some(text);
            }
        }

        Ok(last_text.unwrap_or_else(|| "(sub-agent returned no text result)".into()))
    }
}
