use anyhow::Result;
use async_trait::async_trait;

/// Accumulated token usage across LLM calls
#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

impl TokenUsage {
    pub fn accumulate(&mut self, input: u32, output: u32) {
        self.input_tokens += input as u64;
        self.output_tokens += output as u64;
    }
}

/// Agent → UI event stream
#[derive(Debug, Clone)]
pub enum AgentEvent {
    /// Thinking block started
    ThinkingStart,
    /// Incremental thinking text
    ThinkingDelta(String),
    /// Incremental response text
    TextDelta(String),
    /// Tool call started
    ToolCallStart { id: String, name: String, input_preview: String },
    /// Tool call ended
    ToolCallEnd { id: String, name: String, result: String },
    /// Error message
    Error(String),
    /// Conversation turn ended with usage summary
    Done { usage: Option<TokenUsage> },
}

/// Transport layer abstraction: bridge between Agent and UI
#[async_trait]
pub trait Transport: Send + Sync {
    /// Read user input (blocking)
    async fn read_input(&self) -> Result<String>;
    /// Push agent event to UI
    async fn send_event(&self, event: AgentEvent) -> Result<()>;
}
