use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use tracing::{debug, info};

use crate::llm::error::LlmError;
use crate::llm::provider::LlmProvider;
use crate::llm::types::{ChatRequest, ChatResponse};

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

pub struct AnthropicProvider {
    client: Client,
    api_key: String,
    model: String,
    max_tokens: u32,
}

impl AnthropicProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model,
            max_tokens: 4096,
        }
    }

    pub fn model_name(&self) -> &str {
        &self.model
    }
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    async fn chat(&self, mut request: ChatRequest) -> Result<ChatResponse> {
        request.model = self.model.clone();
        if request.max_tokens == 0 {
            request.max_tokens = self.max_tokens;
        }

        let body = request.to_api();
        debug!("Sending request to Anthropic API, model={}", request.model);

        let resp = self
            .client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| LlmError::RequestFailed(e.to_string()))?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(LlmError::ApiError {
                status: status.as_u16(),
                message: text,
            }
            .into());
        }

        let api_resp: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| LlmError::ParseError(e.to_string()))?;

        debug!("Anthropic API response received, parsing...");

        let typed_resp: crate::llm::types::ApiResponse =
            serde_json::from_value(api_resp).map_err(|e| LlmError::ParseError(e.to_string()))?;

        let chat_response = ChatResponse::try_from(typed_resp)
            .map_err(|e| LlmError::ParseError(e))?;

        info!(
            "LLM response: stop_reason={:?}, input_tokens={}, output_tokens={}",
            chat_response.stop_reason,
            chat_response.usage.input_tokens,
            chat_response.usage.output_tokens
        );

        Ok(chat_response)
    }
}
