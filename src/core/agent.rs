use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::RwLock;
use tokio::sync::mpsc::Receiver;
use tracing::{error, warn};

use crate::config::Config;
use crate::core::message::{Message, MessageContent};
use crate::core::session::SessionManager;
use crate::llm::provider::LlmProvider;
use crate::llm::types::*;
use crate::prompt::PromptManager;
use crate::tools::ToolRegistry;
use crate::transport::AgentEvent;
use crate::utils::tool_input_preview;

pub struct Agent {
    llm: Arc<dyn LlmProvider>,
    tools: Arc<RwLock<ToolRegistry>>,
    prompts: Arc<PromptManager>,
    sessions: SessionManager,
    config: Config,
}

impl Agent {
    pub fn new(
        llm: Arc<dyn LlmProvider>,
        tools: ToolRegistry,
        prompts: Arc<PromptManager>,
        data_dir: PathBuf,
        config: Config,
    ) -> Self {
        Self {
            llm,
            tools: Arc::new(RwLock::new(tools)),
            prompts,
            sessions: SessionManager::new(data_dir),
            config,
        }
    }

    pub fn session_manager(&self) -> SessionManager {
        self.sessions.clone()
    }

    /// 核心 SDK 接口：传入 session_id 则接续会话，None 则自动创建新会话
    /// 返回 (实际 session_id, 事件流)
    pub async fn chat(
        &self,
        session_id: Option<&str>,
        user_message: &str,
        working_dir: &Path,
    ) -> Result<(String, Receiver<AgentEvent>)> {
        let (tx, rx) = tokio::sync::mpsc::channel(32);
        let config = self.config.clone();

        let sid = match session_id {
            Some(id) => id.to_string(),
            None => self.sessions.generate_id(),
        };

        let mut session = self.sessions.get_or_create(&sid, working_dir.to_path_buf()).await?;
        session.add_message(Message::user(user_message));

        let llm = self.llm.clone();
        let tools = self.tools.clone();
        let prompts = self.prompts.clone();
        let sessions = self.sessions.clone();

        tokio::spawn(async move {
            let mut iterations = 0;

            loop {
                iterations += 1;
                let cfg = config.get();
                if iterations > cfg.max_iterations {
                    let _ = tx.send(AgentEvent::Error("超过最大循环次数".into())).await;
                    break;
                }

                let request = match build_request(&session, &prompts, &tools, &cfg).await {
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
                                ContentBlock::Thinking { text } => {
                                    let _ = tx
                                        .send(AgentEvent::Thinking(text.clone()))
                                        .await;
                                    assistant_content
                                        .push(MessageContent::Thinking { text: text.clone() });
                                }
                                ContentBlock::Text { text } => {
                                    let _ = tx
                                        .send(AgentEvent::TextComplete(text.clone()))
                                        .await;
                                    assistant_content
                                        .push(MessageContent::Text { text: text.clone() });
                                }
                                ContentBlock::ToolUse { id, name, input } => {
                                    has_tool_calls = true;
                                    let input_preview = tool_input_preview(name, input);
                                    let _ = tx
                                        .send(AgentEvent::ToolCallStart {
                                            id: id.clone(),
                                            name: name.clone(),
                                            input_preview,
                                        })
                                        .await;

                                    let working_dir = session.working_dir.clone();
                                    let result = {
                                        let guard = tools.read().await;
                                        guard.execute(name, input.clone(), &working_dir).await
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

        Ok((sid, rx))
    }
}

async fn build_request(
    session: &crate::core::session::Session,
    prompts: &PromptManager,
    tools: &RwLock<ToolRegistry>,
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
        max_tokens: cfg.max_tokens,
        system: Some(system),
        messages,
        tools: tool_defs,
        thinking_budget: cfg.thinking_budget,
    })
}
