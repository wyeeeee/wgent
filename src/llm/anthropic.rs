use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use tracing::{debug, info};

use crate::config::Config;
use crate::llm::error::LlmError;
use crate::llm::provider::LlmProvider;
use crate::llm::types::{ChatRequest, ChatResponse};

const ANTHROPIC_VERSION: &str = "2023-06-01";

pub struct AnthropicProvider {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
    config: Config,
}

impl AnthropicProvider {
    pub fn with_base_url(api_key: String, model: String, base_url: String, config: Config) -> Self {
        let base_url = base_url.trim_end_matches('/').to_string();
        Self {
            client: Client::new(),
            api_key,
            base_url,
            model,
            config,
        }
    }

    pub fn model_name(&self) -> &str {
        &self.model
    }
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    async fn chat(&self, mut request: ChatRequest) -> Result<ChatResponse> {
        let cfg = self.config.get();
        request.model = self.model.clone();
        if request.max_tokens == 0 {
            request.max_tokens = cfg.max_tokens;
        }
        if request.thinking_budget == 0 {
            request.thinking_budget = cfg.thinking_budget;
        }

        let body = request.to_api();
        debug!("Sending request to Anthropic API, model={}, thinking_budget={}", request.model, request.thinking_budget);

        let resp = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
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

        let raw = resp
            .text()
            .await
            .map_err(|e| LlmError::ParseError(e.to_string()))?;

        debug!("Raw API response: {raw}");

        let api_resp: serde_json::Value = serde_json::from_str(&raw)
            .map_err(|e| LlmError::ParseError(format!("JSON parse failed: {e}, body: {raw}")))?;

        let typed_resp: crate::llm::types::ApiResponse =
            serde_json::from_value(api_resp).map_err(|e| {
                LlmError::ParseError(format!("Response struct parse failed: {e}, body: {raw}"))
            })?;

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
