use serde_json::Value;
use tracing::debug;

/// A parsed Anthropic SSE event from the streaming API.
#[derive(Debug)]
pub enum SseEvent {
    MessageStart {
        id: String,
        model: String,
        input_tokens: u32,
    },
    ContentBlockStart {
        index: usize,
        block: SseBlock,
    },
    ContentBlockDelta {
        index: usize,
        delta: SseDelta,
    },
    ContentBlockStop {
        index: usize,
    },
    MessageDelta {
        stop_reason: Option<String>,
        output_tokens: u32,
    },
    MessageStop,
    Ping,
    Error {
        error_type: String,
        message: String,
    },
}

#[derive(Debug)]
pub enum SseBlock {
    Text,
    Thinking,
    ToolUse { id: String, name: String },
}

#[derive(Debug)]
pub enum SseDelta {
    Text { text: String },
    Thinking { text: String },
    Signature { signature: String },
    InputJson { partial: String },
}

/// Parse a single SSE event from `event: ...\ndata: ...\n` pair.
/// Returns `None` for unrecognized/unparseable events.
pub fn parse_sse_event(event_type: &str, data: &str) -> Option<SseEvent> {
    let data: Value = serde_json::from_str(data).ok()?;

    match event_type {
        "message_start" => {
            let msg = data.get("message")?;
            Some(SseEvent::MessageStart {
                id: msg.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                model: msg.get("model").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                input_tokens: msg
                    .pointer("/usage/input_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32,
            })
        }
        "content_block_start" => {
            let index = data.get("index")?.as_u64()? as usize;
            let cb = data.get("content_block")?;
            let block_type = cb.get("type")?.as_str()?;

            let block = match block_type {
                "text" => SseBlock::Text,
                "thinking" => SseBlock::Thinking,
                "tool_use" => SseBlock::ToolUse {
                    id: cb.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    name: cb.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                },
                _ => return None,
            };
            Some(SseEvent::ContentBlockStart { index, block })
        }
        "content_block_delta" => {
            let index = data.get("index")?.as_u64()? as usize;
            let delta = data.get("delta")?;
            let delta_type = delta.get("type")?.as_str()?;

            let d = match delta_type {
                "text_delta" => SseDelta::Text {
                    text: delta.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                },
                "thinking_delta" => SseDelta::Thinking {
                    text: delta.get("thinking").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                },
                "signature_delta" => SseDelta::Signature {
                    signature: delta.get("signature").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                },
                "input_json_delta" => SseDelta::InputJson {
                    partial: delta.get("partial_json").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                },
                _ => return None,
            };
            Some(SseEvent::ContentBlockDelta { index, delta: d })
        }
        "content_block_stop" => {
            let index = data.get("index")?.as_u64()? as usize;
            Some(SseEvent::ContentBlockStop { index })
        }
        "message_delta" => {
            let delta = data.get("delta")?;
            let stop_reason = delta
                .get("stop_reason")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let output_tokens = data
                .pointer("/usage/output_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            Some(SseEvent::MessageDelta {
                stop_reason,
                output_tokens,
            })
        }
        "message_stop" => Some(SseEvent::MessageStop),
        "ping" => Some(SseEvent::Ping),
        "error" => {
            let err = data.get("error")?;
            Some(SseEvent::Error {
                error_type: err.get("type").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
                message: err.get("message").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            })
        }
        _ => {
            debug!("SSE: unknown event type '{event_type}', data={}", &data.to_string().chars().take(200).collect::<String>());
            None
        }
    }
}

/// SSE line parser state machine. Feeds raw bytes, emits parsed events.
pub struct SseParser {
    buffer: String,
    event_type: Option<String>,
}

impl SseParser {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            event_type: None,
        }
    }

    /// Feed raw bytes from the HTTP stream. Returns parsed events.
    pub fn feed(&mut self, chunk: &[u8]) -> Vec<SseEvent> {
        let text = String::from_utf8_lossy(chunk);
        self.buffer.push_str(&text);
        self.try_parse()
    }

    /// Flush any remaining buffered data (call when stream ends).
    pub fn flush(&mut self) -> Vec<SseEvent> {
        self.try_parse()
    }

    fn try_parse(&mut self) -> Vec<SseEvent> {
        let mut events = Vec::new();

        while let Some(pos) = self.buffer.find('\n') {
            let line = self.buffer[..pos].trim_end_matches('\r').to_string();
            self.buffer = self.buffer[pos + 1..].to_string();

            if line.is_empty() {
                continue;
            }

            if let Some(data) = line.strip_prefix("data: ") {
                if let Some(event_type) = self.event_type.take() {
                    if let Some(evt) = parse_sse_event(&event_type, data) {
                        events.push(evt);
                    } else {
                        debug!("SSE: failed to parse event '{event_type}'");
                    }
                } else {
                    debug!("SSE: data line without preceding event type");
                }
            } else if let Some(evt) = line.strip_prefix("event: ") {
                self.event_type = Some(evt.to_string());
            }
            // Ignore comment lines (starting with ':') and unknown prefixes
        }

        events
    }
}
