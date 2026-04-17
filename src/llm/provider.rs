use anyhow::Result;
use async_trait::async_trait;

use crate::llm::types::{ChatRequest, ChatResponse};

/// LLM 提供者抽象
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse>;
}
