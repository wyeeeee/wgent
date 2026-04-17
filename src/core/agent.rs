use std::sync::Arc;

use anyhow::Result;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::core::conversation::Conversation;
use crate::core::message::{Message, MessageContent};
use crate::llm::provider::LlmProvider;
use crate::llm::types::{ChatMessage, ChatRequest, ContentBlock, Role};
use crate::prompt::PromptManager;
use crate::tools::ToolRegistry;
use crate::transport::{AgentEvent, Transport};

const MAX_LOOP_ITERATIONS: usize = 50;

pub struct Agent {
    llm: Arc<dyn LlmProvider>,
    tools: Arc<RwLock<ToolRegistry>>,
    transport: Arc<dyn Transport>,
    prompts: Arc<PromptManager>,
    conversation: Conversation,
}

impl Agent {
    pub fn new(
        llm: Arc<dyn LlmProvider>,
        tools: ToolRegistry,
        transport: Arc<dyn Transport>,
        prompts: Arc<PromptManager>,
        max_history: usize,
    ) -> Self {
        Self {
            llm,
            tools: Arc::new(RwLock::new(tools)),
            transport,
            prompts,
            conversation: Conversation::new(max_history),
        }
    }

    /// 主循环：持续读取用户输入并处理
    pub async fn run(&mut self) -> Result<()> {
        info!("Agent started, waiting for user input...");

        loop {
            let input = match self.transport.read_input().await {
                Ok(input) => input,
                Err(e) => {
                    error!("Failed to read input: {e}");
                    break;
                }
            };

            if input.trim().is_empty() {
                continue;
            }

            if self.handle_turn(&input).await.is_err() {
                break;
            }
        }

        Ok(())
    }

    /// 处理单轮对话
    async fn handle_turn(&mut self, user_input: &str) -> Result<()> {
        self.conversation.add_message(Message::user(user_input));

        let mut iterations = 0;

        loop {
            iterations += 1;
            if iterations > MAX_LOOP_ITERATIONS {
                self.transport
                    .send_event(AgentEvent::Error(
                        "超过最大循环次数限制".to_string(),
                    ))
                    .await?;
                break;
            }

            let request = self.build_request()?;

            let response = match self.llm.chat(request).await {
                Ok(resp) => resp,
                Err(e) => {
                    error!("LLM request failed: {e}");
                    self.transport
                        .send_event(AgentEvent::Error(format!("LLM 请求失败: {e}")))
                        .await?;
                    break;
                }
            };

            // 处理响应内容块
            let mut assistant_content = Vec::new();
            let mut has_tool_calls = false;

            for block in &response.content {
                match block {
                    ContentBlock::Text { text } => {
                        self.transport
                            .send_event(AgentEvent::TextComplete(text.clone()))
                            .await?;
                        assistant_content.push(MessageContent::Text(text.clone()));
                    }
                    ContentBlock::ToolUse { id, name, input } => {
                        has_tool_calls = true;
                        self.transport
                            .send_event(AgentEvent::ToolCallStart {
                                id: id.clone(),
                                name: name.clone(),
                            })
                            .await?;

                        let result = self.execute_tool(name, input.clone()).await;

                        let output = match result {
                            Ok(output) => output,
                            Err(e) => {
                                warn!("Tool '{name}' failed: {e}");
                                self.prompts
                                    .render_tool_error(name, &e.to_string())
                                    .unwrap_or_else(|_| e.to_string())
                            }
                        };

                        self.transport
                            .send_event(AgentEvent::ToolCallEnd {
                                id: id.clone(),
                                name: name.clone(),
                                result: output.clone(),
                            })
                            .await?;

                        assistant_content.push(MessageContent::ToolCall {
                            id: id.clone(),
                            name: name.clone(),
                            arguments: input.clone(),
                        });
                        assistant_content.push(MessageContent::ToolResult {
                            tool_use_id: id.clone(),
                            output,
                        });
                    }
                    ContentBlock::ToolResult { .. } => {}
                }
            }

            self.conversation
                .add_message(Message {
                    role: Role::Assistant,
                    content: assistant_content,
                });

            if !has_tool_calls {
                self.transport.send_event(AgentEvent::Done).await?;
                break;
            }
        }

        Ok(())
    }

    async fn execute_tool(
        &self,
        name: &str,
        input: serde_json::Value,
    ) -> Result<String> {
        let tools = self.tools.read().await;
        tools.execute(name, input).await
    }

    fn build_request(&self) -> Result<ChatRequest> {
        let system = self
            .prompts
            .render_system("Agent", None::<&str>, &[])?;

        let tools = self.tools.try_read()
            .map(|t| t.definitions())
            .unwrap_or_default();

        let messages = self
            .conversation
            .messages()
            .iter()
            .map(|msg| ChatMessage {
                role: msg.role.clone(),
                content: msg.content.iter().map(|c| match c {
                    MessageContent::Text(text) => ContentBlock::Text { text: text.clone() },
                    MessageContent::ToolCall { id, name, arguments } => ContentBlock::ToolUse {
                        id: id.clone(),
                        name: name.clone(),
                        input: arguments.clone(),
                    },
                    MessageContent::ToolResult { tool_use_id, output } => ContentBlock::ToolResult {
                        tool_use_id: tool_use_id.clone(),
                        content: output.clone(),
                    },
                }).collect(),
            })
            .collect();

        Ok(ChatRequest {
            model: String::new(),
            max_tokens: 0,
            system: Some(system),
            messages,
            tools,
        })
    }
}
