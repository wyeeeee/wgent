use anyhow::Result;
use async_trait::async_trait;

use crate::llm::types::{ChatRequest, ChatResponse};

/// LLM provider abstraction
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse>;
}
