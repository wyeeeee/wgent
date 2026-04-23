use async_trait::async_trait;

use crate::llm::error::LlmError;
use crate::llm::types::ChatRequest;

/// LLM provider abstraction — always returns a streaming response.
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn chat(&self, request: ChatRequest) -> Result<reqwest::Response, LlmError>;
}
