use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::sync::RwLock;
use tracing::warn;

use crate::config::Config;
use crate::core::message::{Message, MessageContent};
use crate::core::session::Session;
use crate::llm::provider::LlmProvider;
use crate::llm::types::*;
use crate::prompt::PromptManager;
use crate::tools::tool::{Tool, ToolContext};
use crate::tools::ToolRegistry;

pub struct SubAgentTool {
    llm: Arc<dyn LlmProvider>,
    config: Config,
    prompts: Arc<PromptManager>,
}

impl SubAgentTool {
    pub fn new(
        llm: Arc<dyn LlmProvider>,
        config: Config,
        prompts: Arc<PromptManager>,
    ) -> Self {
        Self { llm, config, prompts }
    }
}

#[async_trait]
impl Tool for SubAgentTool {
    fn name(&self) -> &str {
        "subagent"
    }

    fn description(&self) -> &str {
        "生成子代理执行子任务。子代理可独立使用 read/write/edit/bash 工具，完成后返回结果文本。不可递归调用。"
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

        // 基础工具（不含 subagent，防止递归）
        let tools = Arc::new(RwLock::new(ToolRegistry::from_config(
            &self.config,
            "read,write,edit,bash",
        )));

        let cfg = self.config.get();

        // 临时 session，不持久化
        let mut session = Session::new(format!("sub_{}", now_millis()), ctx.working_dir.clone());
        session.add_message(Message::user(task));

        let mut final_text = String::new();

        for _ in 0..cfg.max_iterations {
            let request = build_sub_request(&session, &self.prompts, &tools, &cfg).await?;
            let response = match self.llm.chat(request).await {
                Ok(r) => r,
                Err(e) => return Err(anyhow!("子代理 LLM 请求失败: {e}")),
            };

            let mut has_tool_calls = false;
            let mut assistant_content = Vec::new();
            let mut tool_results = Vec::new();

            for block in &response.content {
                match block {
                    ContentBlock::Thinking { text } => {
                        assistant_content.push(MessageContent::Thinking { text: text.clone() });
                    }
                    ContentBlock::Text { text } => {
                        final_text = text.clone();
                        assistant_content.push(MessageContent::Text { text: text.clone() });
                    }
                    ContentBlock::ToolUse { id, name, input } => {
                        has_tool_calls = true;

                        let tool_ctx = ToolContext {
                            working_dir: ctx.working_dir.clone(),
                            events: None,
                        };

                        let output = {
                            let guard = tools.read().await;
                            guard.execute(name, input.clone(), &tool_ctx).await
                        }
                        .unwrap_or_else(|e| {
                            warn!("SubAgent tool '{name}' failed: {e}");
                            format!("Error: {e}")
                        });

                        assistant_content.push(MessageContent::ToolCall {
                            id: id.clone(),
                            name: name.clone(),
                            arguments: input.clone(),
                        });
                        tool_results.push(MessageContent::ToolResult {
                            tool_use_id: id.clone(),
                            output,
                        });
                    }
                    ContentBlock::ToolResult { .. } => {}
                }
            }

            session.add_message(Message {
                role: Role::Assistant,
                content: assistant_content,
            });

            if has_tool_calls {
                session.add_message(Message {
                    role: Role::User,
                    content: tool_results,
                });
            } else {
                break;
            }
        }

        if final_text.is_empty() {
            final_text = "(子代理未返回文本结果)".into();
        }

        Ok(final_text)
    }
}

fn now_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

async fn build_sub_request(
    session: &Session,
    prompts: &PromptManager,
    tools: &Arc<RwLock<ToolRegistry>>,
    cfg: &crate::config::ConfigValues,
) -> Result<ChatRequest> {
    let system = prompts.render_system("Wgent", None::<&str>, &[], &session.working_dir)?;
    let tool_defs = tools.read().await.definitions();

    let messages = session
        .messages
        .iter()
        .map(|msg| ChatMessage {
            role: msg.role.clone(),
            content: msg
                .content
                .iter()
                .map(|c| match c {
                    MessageContent::Thinking { text } => ContentBlock::Thinking { text: text.clone() },
                    MessageContent::Text { text } => ContentBlock::Text { text: text.clone() },
                    MessageContent::ToolCall { id, name, arguments } => ContentBlock::ToolUse {
                        id: id.clone(),
                        name: name.clone(),
                        input: arguments.clone(),
                    },
                    MessageContent::ToolResult { tool_use_id, output } => ContentBlock::ToolResult {
                        tool_use_id: tool_use_id.clone(),
                        content: output.clone(),
                    },
                })
                .collect(),
        })
        .collect();

    Ok(ChatRequest {
        model: String::new(),
        max_tokens: cfg.max_tokens,
        system: Some(system),
        messages,
        tools: tool_defs,
        thinking_budget: cfg.thinking_budget,
    })
}
