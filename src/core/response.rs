use std::sync::Arc;

use serde_json::Value;
use tokio::sync::mpsc::Sender;
use tracing::{error, warn};

use crate::core::message::{Message, MessageContent};
use crate::core::session::Session;
use crate::llm::sse::{SseBlock, SseDelta, SseEvent, SseParser};
use crate::llm::types::*;
use crate::prompt::PromptManager;
use crate::tools::tool::ToolContext;
use crate::tools::ToolRegistry;
use crate::transport::{AgentEvent, TokenUsage};
use crate::utils::tool_input_preview;

struct ToolCallResult {
    id: String,
    output: String,
}

/// Tracks the type of the currently active content block.
#[derive(Clone, Copy, PartialEq)]
enum BlockType {
    None,
    Thinking,
    Text,
    ToolUse,
}

struct StreamOutput {
    assistant_content: Vec<MessageContent>,
    usage: TokenUsage,
    stop_reason: Option<StopReason>,
    completed: bool, // false if transport channel closed mid-stream
}

/// Process a streaming HTTP response from the LLM.
/// Returns (accumulated usage, whether tool calls need another loop iteration).
pub async fn process_response(
    response: reqwest::Response,
    session: &mut Session,
    tools: &Arc<ToolRegistry>,
    prompts: &Arc<PromptManager>,
    tx: &Sender<AgentEvent>,
) -> (TokenUsage, bool) {
    let stream_output = parse_stream(response, tx).await;

    if !stream_output.completed {
        return (stream_output.usage, false);
    }

    let tool_calls: Vec<(String, String, Value)> = stream_output
        .assistant_content
        .iter()
        .filter_map(|c| match c {
            MessageContent::ToolCall { id, name, arguments } => {
                Some((id.clone(), name.clone(), arguments.clone()))
            }
            _ => None,
        })
        .collect();

    let has_tool_calls = !tool_calls.is_empty();

    session.add_message(Message {
        role: Role::Assistant,
        content: stream_output.assistant_content,
    });

    if has_tool_calls {
        let results = execute_tool_calls(
            tool_calls,
            tools,
            prompts,
            tx,
            session.working_dir.clone(),
        )
        .await;

        session.add_message(Message {
            role: Role::User,
            content: results
                .into_iter()
                .map(|r| MessageContent::ToolResult {
                    tool_use_id: r.id,
                    output: r.output,
                })
                .collect(),
        });
    }

    if stream_output.stop_reason == Some(StopReason::MaxTokens) {
        let _ = tx.send(AgentEvent::Error(
            "Output truncated: max_tokens limit reached. Consider increasing max_tokens in config."
                .to_string(),
        )).await;
    }

    (stream_output.usage, has_tool_calls)
}

/// Parse the SSE byte stream, dispatch real-time events to transport,
/// and return accumulated content blocks, usage, and stop reason.
async fn parse_stream(
    response: reqwest::Response,
    tx: &Sender<AgentEvent>,
) -> StreamOutput {
    let mut parser = SseParser::new();
    let mut usage = TokenUsage::default();
    let mut assistant_content: Vec<MessageContent> = Vec::new();

    let mut block_type = BlockType::None;
    let mut thinking_text = String::new();
    let mut text_content = String::new();
    let mut tool_id = String::new();
    let mut tool_name = String::new();
    let mut tool_json_buffer = String::new();
    let mut stop_reason: Option<StopReason> = None;

    let mut stream = response.bytes_stream();
    use futures_util::StreamExt;

    while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(c) => c,
            Err(e) => {
                error!("Stream read error: {e}");
                break;
            }
        };

        let events = parser.feed(&chunk);
        for event in events {
            match event {
                SseEvent::MessageStart { input_tokens, .. } => {
                    usage.input_tokens += input_tokens as u64;
                }
                SseEvent::ContentBlockStart { block, .. } => {
                    match block {
                        SseBlock::Thinking => {
                            block_type = BlockType::Thinking;
                            thinking_text.clear();
                            let _ = tx.send(AgentEvent::ThinkingStart).await;
                        }
                        SseBlock::Text => {
                            block_type = BlockType::Text;
                            text_content.clear();
                        }
                        SseBlock::ToolUse { id, name } => {
                            block_type = BlockType::ToolUse;
                            tool_id = id;
                            tool_name = name;
                            tool_json_buffer.clear();
                        }
                    }
                }
                SseEvent::ContentBlockDelta { delta, .. } => match delta {
                    SseDelta::Text { text } => {
                        if tx.send(AgentEvent::TextDelta(text.clone())).await.is_err() {
                            return StreamOutput {
                                assistant_content,
                                usage,
                                stop_reason,
                                completed: false,
                            };
                        }
                        text_content.push_str(&text);
                    }
                    SseDelta::Thinking { text } => {
                        if tx.send(AgentEvent::ThinkingDelta(text.clone())).await.is_err() {
                            return StreamOutput {
                                assistant_content,
                                usage,
                                stop_reason,
                                completed: false,
                            };
                        }
                        thinking_text.push_str(&text);
                    }
                    SseDelta::Signature { .. } => {}
                    SseDelta::InputJson { partial } => {
                        tool_json_buffer.push_str(&partial);
                    }
                },
                SseEvent::ContentBlockStop { .. } => {
                    match block_type {
                        BlockType::Thinking => {
                            if !thinking_text.is_empty() {
                                assistant_content.push(MessageContent::Thinking {
                                    text: std::mem::take(&mut thinking_text),
                                });
                            }
                        }
                        BlockType::Text => {
                            if !text_content.is_empty() {
                                assistant_content.push(MessageContent::Text {
                                    text: std::mem::take(&mut text_content),
                                });
                            }
                        }
                        BlockType::ToolUse => {
                            let input = parse_tool_input(&tool_json_buffer, &tool_name);
                            let input_preview = tool_input_preview(&tool_name, &input);
                            if tx
                                .send(AgentEvent::ToolCallStart {
                                    id: tool_id.clone(),
                                    name: tool_name.clone(),
                                    input_preview,
                                })
                                .await
                                .is_err()
                            {
                                return StreamOutput {
                                    assistant_content,
                                    usage,
                                    stop_reason,
                                    completed: false,
                                };
                            }
                            assistant_content.push(MessageContent::ToolCall {
                                id: tool_id.clone(),
                                name: tool_name.clone(),
                                arguments: input,
                            });
                            tool_json_buffer.clear();
                        }
                        BlockType::None => {}
                    }
                    block_type = BlockType::None;
                }
                SseEvent::MessageDelta {
                    stop_reason: sr,
                    input_tokens,
                    output_tokens,
                } => {
                    usage.input_tokens += input_tokens as u64;
                    usage.output_tokens += output_tokens as u64;
                    if let Some(s) = sr {
                        stop_reason = match s.as_str() {
                            "end_turn" => Some(StopReason::EndTurn),
                            "tool_use" => Some(StopReason::ToolUse),
                            "max_tokens" => Some(StopReason::MaxTokens),
                            _ => None,
                        };
                    }
                }
                SseEvent::MessageStop => {}
                SseEvent::Ping => {}
                SseEvent::Error { message, .. } => {
                    error!("Stream error from API: {message}");
                    let _ = tx.send(AgentEvent::Error(message)).await;
                }
            }
        }
    }

    for event in parser.flush() {
        if let SseEvent::MessageDelta { output_tokens, .. } = event {
            usage.output_tokens += output_tokens as u64;
        }
    }

    StreamOutput {
        assistant_content,
        usage,
        stop_reason,
        completed: true,
    }
}

/// Execute tool calls in parallel, return results per call.
async fn execute_tool_calls(
    tool_calls: Vec<(String, String, Value)>,
    tools: &Arc<ToolRegistry>,
    prompts: &Arc<PromptManager>,
    tx: &Sender<AgentEvent>,
    working_dir: std::path::PathBuf,
) -> Vec<ToolCallResult> {
    let mut handles = Vec::with_capacity(tool_calls.len());
    for (id, name, arguments) in tool_calls {
        let tools = tools.clone();
        let prompts = prompts.clone();
        let tx = tx.clone();
        let working_dir = working_dir.clone();

        handles.push(tokio::spawn(async move {
            let ctx = ToolContext {
                working_dir,
                events: Some(tx.clone()),
            };
            let result = tools.execute(&name, arguments.clone(), &ctx).await;
            let output = match result {
                Ok(o) => o,
                Err(e) => {
                    warn!("Tool '{}' failed: {e}", name);
                    prompts
                        .render_tool_error(&name, &e.to_string())
                        .unwrap_or_else(|_| e.to_string())
                }
            };

            if tx
                .send(AgentEvent::ToolCallEnd {
                    id: id.clone(),
                    name: name.clone(),
                    result: output.clone(),
                })
                .await
                .is_err()
            {
                warn!("Channel closed while sending ToolCallEnd for '{}'", name);
            }

            ToolCallResult { id, output }
        }));
    }

    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        match handle.await {
            Ok(r) => results.push(r),
            Err(e) => error!("Tool task panicked: {e}"),
        }
    }
    results
}

fn parse_tool_input(json_buffer: &str, tool_name: &str) -> Value {
    if json_buffer.is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_str(json_buffer).unwrap_or_else(|_| {
            warn!("Failed to parse tool input JSON for {}", tool_name);
            serde_json::json!({})
        })
    }
}
