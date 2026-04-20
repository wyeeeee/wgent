use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Result, bail};
use tokio::sync::RwLock;
use tokio::sync::mpsc::{Receiver, Sender};
use tracing::{error, warn};

use crate::commands::CommandRegistry;
use crate::config::Config;
use crate::core::message::{Message, MessageContent};
use crate::core::session::SessionManager;
use crate::llm::provider::LlmProvider;
use crate::llm::types::*;
use crate::prompt::PromptManager;
use crate::tools::tool::ToolContext;
use crate::tools::ToolRegistry;
use crate::transport::AgentEvent;
use crate::utils::tool_input_preview;

pub struct Agent {
    llm: Arc<dyn LlmProvider>,
    tools: Arc<RwLock<ToolRegistry>>,
    commands: Arc<RwLock<CommandRegistry>>,
    prompts: Arc<PromptManager>,
    sessions: SessionManager,
    config: Config,
    working_dir: PathBuf,
}

impl Agent {
    pub fn new(dir: &Path, working_dir: &Path) -> Result<Self> {
        let config = Config::load(dir)?;
        let cfg = config.get();

        if cfg.api_key.is_empty() {
            bail!(
                "API key 未设置，请编辑 {}/wgent.json",
                dir.display()
            );
        }

        let llm = Arc::new(crate::llm::AnthropicProvider::new(config.clone()));
        let prompts = Arc::new(PromptManager::new()?);
        let mut tools = ToolRegistry::from_config(&config, &cfg.tools);

        // SubAgentTool 需要依赖 LLM/Prompts，单独注册
        let spec = cfg.tools.to_lowercase();
        if spec.trim() == "all" || spec.split(',').any(|s| s.trim() == "subagent") {
            tools.register(Box::new(crate::tools::builtin::SubAgentTool::new(
                llm.clone(),
                config.clone(),
                prompts.clone(),
            )));
        }

        let commands = CommandRegistry::from_config(&cfg.commands);
        let sessions = SessionManager::new(dir.join("sessions"));

        Ok(Self {
            llm,
            tools: Arc::new(RwLock::new(tools)),
            commands: Arc::new(RwLock::new(commands)),
            prompts,
            sessions,
            config,
            working_dir: working_dir.to_path_buf(),
        })
    }

    pub fn session_manager(&self) -> SessionManager {
        self.sessions.clone()
    }

    pub fn commands(&self) -> Arc<RwLock<CommandRegistry>> {
        self.commands.clone()
    }

    pub fn working_dir(&self) -> &Path {
        &self.working_dir
    }

    pub fn model_name(&self) -> String {
        self.config.get().model
    }

    pub async fn chat(
        &self,
        session_id: Option<&str>,
        user_message: &str,
    ) -> Result<(String, Receiver<AgentEvent>)> {
        let (tx, rx) = tokio::sync::mpsc::channel(32);

        let sid = match session_id {
            Some(id) => id.to_string(),
            None => self.sessions.generate_id(),
        };

        let mut session = self
            .sessions
            .get_or_create(&sid, self.working_dir.to_path_buf())
            .await?;
        session.add_message(Message::user(user_message));

        let llm = self.llm.clone();
        let tools = self.tools.clone();
        let prompts = self.prompts.clone();
        let config = self.config.clone();
        let sessions = self.sessions.clone();

        tokio::spawn(async move {
            run_loop(llm, tools, prompts, config, &mut session, &tx).await;
            let _ = tx.send(AgentEvent::Done).await;
            let _ = sessions.save(&session).await;
        });

        Ok((sid, rx))
    }
}

async fn run_loop(
    llm: Arc<dyn LlmProvider>,
    tools: Arc<RwLock<ToolRegistry>>,
    prompts: Arc<PromptManager>,
    config: Config,
    session: &mut crate::core::session::Session,
    tx: &Sender<AgentEvent>,
) {
    let mut iterations = 0;

    loop {
        iterations += 1;
        let cfg = config.get();
        if iterations > cfg.max_iterations {
            let _ = tx.send(AgentEvent::Error("超过最大循环次数".into())).await;
            return;
        }

        let request = match build_request(session, &prompts, &tools, &cfg).await {
            Ok(r) => r,
            Err(e) => {
                let _ = tx.send(AgentEvent::Error(format!("构建请求失败: {e}"))).await;
                return;
            }
        };

        let response = match llm.chat(request).await {
            Ok(r) => r,
            Err(e) => {
                error!("LLM request failed: {e}");
                let _ = tx.send(AgentEvent::Error(format!("LLM 请求失败: {e}"))).await;
                return;
            }
        };

        if !process_response(response, session, &tools, &prompts, tx).await {
            return;
        }
    }
}

struct ToolCall {
    id: String,
    name: String,
    input: serde_json::Value,
}

struct ToolCallResult {
    id: String,
    name: String,
    arguments: serde_json::Value,
    output: String,
}

async fn process_response(
    response: ChatResponse,
    session: &mut crate::core::session::Session,
    tools: &Arc<RwLock<ToolRegistry>>,
    prompts: &Arc<PromptManager>,
    tx: &Sender<AgentEvent>,
) -> bool {
    let mut assistant_content: Vec<MessageContent> = Vec::new();
    let mut tool_calls: Vec<ToolCall> = Vec::new();

    // 阶段 1: 处理文本/思考块，收集工具调用
    for block in &response.content {
        match block {
            ContentBlock::Thinking { text } => {
                let _ = tx.send(AgentEvent::Thinking(text.clone())).await;
                assistant_content.push(MessageContent::Thinking { text: text.clone() });
            }
            ContentBlock::Text { text } => {
                let _ = tx.send(AgentEvent::TextComplete(text.clone())).await;
                assistant_content.push(MessageContent::Text { text: text.clone() });
            }
            ContentBlock::ToolUse { id, name, input } => {
                let input_preview = tool_input_preview(name, input);
                let _ = tx
                    .send(AgentEvent::ToolCallStart {
                        id: id.clone(),
                        name: name.clone(),
                        input_preview,
                    })
                    .await;

                tool_calls.push(ToolCall {
                    id: id.clone(),
                    name: name.clone(),
                    input: input.clone(),
                });
            }
            ContentBlock::ToolResult { .. } => {}
        }
    }

    // 阶段 2: 并行执行所有工具调用
    let tool_results = if tool_calls.is_empty() {
        Vec::new()
    } else {
        let mut handles = Vec::with_capacity(tool_calls.len());

        for tc in tool_calls {
            let tools = tools.clone();
            let prompts = prompts.clone();
            let tx = tx.clone();
            let working_dir = session.working_dir.clone();

            handles.push(tokio::spawn(async move {
                let ctx = ToolContext {
                    working_dir,
                    events: Some(tx.clone()),
                };
                let result = {
                    let guard = tools.read().await;
                    guard.execute(&tc.name, tc.input.clone(), &ctx).await
                };

                let output = match result {
                    Ok(o) => o,
                    Err(e) => {
                        warn!("Tool '{}' failed: {e}", tc.name);
                        prompts
                            .render_tool_error(&tc.name, &e.to_string())
                            .unwrap_or_else(|_| e.to_string())
                    }
                };

                let _ = tx
                    .send(AgentEvent::ToolCallEnd {
                        id: tc.id.clone(),
                        name: tc.name.clone(),
                        result: output.clone(),
                    })
                    .await;

                ToolCallResult {
                    id: tc.id,
                    name: tc.name,
                    arguments: tc.input,
                    output,
                }
            }));
        }

        let mut results = Vec::with_capacity(handles.len());
        for handle in handles {
            match handle.await {
                Ok(r) => results.push(r),
                Err(e) => {
                    error!("Tool task panicked: {e}");
                }
            }
        }
        results
    };

    // 阶段 3: 写入 session
    for r in &tool_results {
        assistant_content.push(MessageContent::ToolCall {
            id: r.id.clone(),
            name: r.name.clone(),
            arguments: r.arguments.clone(),
        });
    }

    session.add_message(Message {
        role: Role::Assistant,
        content: assistant_content,
    });

    let has_tool_calls = !tool_results.is_empty();
    if has_tool_calls {
        session.add_message(Message {
            role: Role::User,
            content: tool_results
                .into_iter()
                .map(|r| MessageContent::ToolResult {
                    tool_use_id: r.id,
                    output: r.output,
                })
                .collect(),
        });
    }

    has_tool_calls
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
