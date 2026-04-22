use std::sync::Arc;

use serde_json::Value;
use tokio::sync::RwLock;
use tokio::sync::mpsc::Sender;
use tracing::{error, warn};

use crate::core::message::{Message, MessageContent};
use crate::core::session::Session;
use crate::llm::types::*;
use crate::prompt::PromptManager;
use crate::tools::tool::ToolContext;
use crate::tools::ToolRegistry;
use crate::transport::AgentEvent;
use crate::utils::tool_input_preview;

struct ToolCall {
    id: String,
    name: String,
    input: Value,
}

struct ToolCallResult {
    id: String,
    name: String,
    arguments: Value,
    output: String,
}

pub async fn process_response(
    response: ChatResponse,
    session: &mut Session,
    tools: &Arc<RwLock<ToolRegistry>>,
    prompts: &Arc<PromptManager>,
    tx: &Sender<AgentEvent>,
) -> bool {
    let mut assistant_content: Vec<MessageContent> = Vec::new();
    let mut tool_calls: Vec<ToolCall> = Vec::new();

    // Phase 1: process text/thinking blocks, collect tool calls
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

    // Phase 2: execute all tool calls in parallel
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

    // Phase 3: write to session
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
