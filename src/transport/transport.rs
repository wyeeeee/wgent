use anyhow::Result;
use async_trait::async_trait;

/// Agent → UI event stream
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum AgentEvent {
    /// Model is thinking (optional display)
    Thinking(String),
    /// Text stream delta
    TextDelta(String),
    /// Complete text response
    TextComplete(String),
    /// Tool call started
    ToolCallStart { id: String, name: String, input_preview: String },
    /// Tool call ended
    ToolCallEnd { id: String, name: String, result: String },
    /// Error message
    Error(String),
    /// Conversation turn ended
    Done,
}

/// Transport layer abstraction: bridge between Agent and UI
#[async_trait]
pub trait Transport: Send + Sync {
    /// Read user input (blocking)
    async fn read_input(&self) -> Result<String>;
    /// Push agent event to UI
    async fn send_event(&self, event: AgentEvent) -> Result<()>;
}
