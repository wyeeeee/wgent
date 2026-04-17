use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::RwLock;
use tokio::sync::mpsc::Receiver;
use tracing::{error, warn};

use crate::core::message::{Message, MessageContent};
use crate::core::session::SessionManager;
use crate::llm::provider::LlmProvider;
use crate::llm::types::*;
use crate::prompt::PromptManager;
use crate::tools::ToolRegistry;
use crate::transport::AgentEvent;

const MAX_LOOP_ITERATIONS: usize = 50;

pub struct Agent {
    llm: Arc<dyn LlmProvider>,
    tools: Arc<RwLock<ToolRegistry>>,
    prompts: Arc<PromptManager>,
    sessions: SessionManager,
}

impl Agent {
    pub fn new(
        llm: Arc<dyn LlmProvider>,
        tools: ToolRegistry,
        prompts: Arc<PromptManager>,
        data_dir: PathBuf,
    ) -> Self {
        Self {
            llm,
            tools: Arc::new(RwLock::new(tools)),
            prompts,
            sessions: SessionManager::new(data_dir),
        }
    }

    /// 核心 SDK 接口：接收 session_id 和用户消息，返回事件流
    pub async fn chat(
        &self,
        session_id: &str,
        user_message: &str,
    ) -> Result<Receiver<AgentEvent>> {
        let (tx, rx) = tokio::sync::mpsc::channel(32);

        let mut session = self.sessions.get_or_create(session_id).await?;
        session.add_message(Message::user(user_message));

        let llm = self.llm.clone();
        let tools = self.tools.clone();
        let prompts = self.prompts.clone();
        let sessions = self.sessions.clone();

        tokio::spawn(async move {
            let mut iterations = 0;

            loop {
                iterations += 1;
                if iterations > MAX_LOOP_ITERATIONS {
                    let _ = tx.send(AgentEvent::Error("超过最大循环次数".into())).await;
                    break;
                }

                let request = match build_request(&session, &prompts, &tools).await {
                    Ok(r) => r,
                    Err(e) => {
                        let _ = tx
                            .send(AgentEvent::Error(format!("构建请求失败: {e}")))
                            .await;
                        break;
                    }
                };

                match llm.chat(request).await {
                    Ok(response) => {
                        let mut has_tool_calls = false;
                        let mut assistant_content = Vec::new();
                        let mut tool_results = Vec::new();

                        for block in &response.content {
                            match block {
                                ContentBlock::Text { text } => {
                                    let _ = tx
                                        .send(AgentEvent::TextComplete(text.clone()))
                                        .await;
                                    assistant_content
                                        .push(MessageContent::Text { text: text.clone() });
                                }
                                ContentBlock::ToolUse { id, name, input } => {
                                    has_tool_calls = true;
                                    let _ = tx
                                        .send(AgentEvent::ToolCallStart {
                                            id: id.clone(),
                                            name: name.clone(),
                                        })
                                        .await;

                                    let result = {
                                        let guard = tools.read().await;
                                        guard.execute(name, input.clone()).await
                                    };

                                    let output = match result {
                                        Ok(o) => o,
                                        Err(e) => {
                                            warn!("Tool '{name}' failed: {e}");
                                            prompts
                                                .render_tool_error(name, &e.to_string())
                                                .unwrap_or_else(|_| e.to_string())
                                        }
                                    };

                                    let _ = tx
                                        .send(AgentEvent::ToolCallEnd {
                                            id: id.clone(),
                                            name: name.clone(),
                                            result: output.clone(),
                                        })
                                        .await;

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
                    Err(e) => {
                        error!("LLM request failed: {e}");
                        let _ = tx
                            .send(AgentEvent::Error(format!("LLM 请求失败: {e}")))
                            .await;
                        break;
                    }
                }
            }

            let _ = tx.send(AgentEvent::Done).await;
            let _ = sessions.save(&session).await;
        });

        Ok(rx)
    }
}

async fn build_request(
    session: &crate::core::session::Session,
    prompts: &PromptManager,
    tools: &RwLock<ToolRegistry>,
) -> Result<ChatRequest> {
    let system = prompts.render_system("Agent", None::<&str>, &[])?;
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
                    MessageContent::Text { text } => ContentBlock::Text { text: text.clone() },
                    MessageContent::ToolCall { id, name, arguments } => ContentBlock::ToolUse {
                        id: id.clone(),
                        name: name.clone(),
                        input: arguments.clone(),
                    },
                    MessageContent::ToolResult { tool_use_id, output } => {
                        ContentBlock::ToolResult {
                            tool_use_id: tool_use_id.clone(),
                            content: output.clone(),
                        }
                    }
                })
                .collect(),
        })
        .collect();

    Ok(ChatRequest {
        model: String::new(),
        max_tokens: 0,
        system: Some(system),
        messages,
        tools: tool_defs,
    })
}
