use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use tracing::{debug, info, warn};

use crate::config::Config;
use crate::llm::error::LlmError;
use crate::llm::provider::LlmProvider;
use crate::llm::types::{ChatRequest, ChatResponse};

const ANTHROPIC_VERSION: &str = "2023-06-01";

pub struct AnthropicProvider {
    client: Client,
    config: Config,
}

impl AnthropicProvider {
    pub fn new(config: Config) -> Self {
        Self {
            client: Client::new(),
            config,
        }
    }

    pub fn model_name(&self) -> String {
        self.config.get().model.clone()
    }
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    async fn chat(&self, mut request: ChatRequest) -> Result<ChatResponse, LlmError> {
        let cfg = self.config.get();
        if request.max_tokens == 0 {
            request.max_tokens = cfg.max_tokens;
        }
        if request.thinking_budget == 0 {
            request.thinking_budget = cfg.thinking_budget;
        }
        let max_retries = cfg.llm_max_retries;

        let mut attempt = 0;
        loop {
            attempt += 1;
            match self.send_request(&request).await {
                Ok(resp) => return Ok(resp),
                Err(e) => {
                    if !e.is_retryable() || attempt > max_retries {
                        return Err(e);
                    }
                    let delay = match e.suggested_delay_ms() {
                        Some(ms) => Duration::from_millis(ms.min(30_000)),
                        None => Duration::from_millis((500 * 2u64.pow(attempt as u32 - 1)).min(10_000)),
                    };
                    warn!(
                        "LLM request failed (attempt {attempt}/{max_retries}): {e}, retrying in {}ms",
                        delay.as_millis()
                    );
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }
}

impl AnthropicProvider {
    async fn send_request(&self, request: &ChatRequest) -> Result<ChatResponse, LlmError> {
        let cfg = self.config.get();
        let body = request.to_api();
        debug!("Sending request to Anthropic API, model={}, thinking_budget={}", request.model, request.thinking_budget);

        let resp = self
            .client
            .post(format!("{}/v1/messages", cfg.base_url.trim_end_matches('/')))
            .header("x-api-key", &cfg.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| LlmError::Network { message: e.to_string() })?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(classify_http_error(status.as_u16(), &text));
        }

        let raw = resp
            .text()
            .await
            .map_err(|e| LlmError::Network { message: e.to_string() })?;

        debug!("Raw API response: {raw}");

        let api_resp: crate::llm::types::ApiResponse = serde_json::from_str(&raw)
            .map_err(|e| LlmError::Parse { message: format!("Failed to parse response: {e}") })?;

        let chat_response = ChatResponse::try_from(api_resp)
            .map_err(|e| LlmError::Parse { message: format!("Failed to convert response: {e}") })?;

        info!(
            "LLM response: stop_reason={:?}, input_tokens={}, output_tokens={}",
            chat_response.stop_reason,
            chat_response.usage.input_tokens,
            chat_response.usage.output_tokens
        );

        Ok(chat_response)
    }
}

fn classify_http_error(status: u16, body: &str) -> LlmError {
    match status {
        429 => {
            let retry_after_ms = None; // Anthropic doesn't use retry-after header typically
            LlmError::RateLimited {
                retry_after_ms,
                message: format!("status=429, body={body}"),
            }
        }
        401 | 403 => LlmError::Authentication {
            message: format!("status={status}, body={body}"),
        },
        400 => LlmError::BadRequest {
            message: format!("status=400, body={body}"),
        },
        404 => LlmError::NotFound {
            message: format!("status=404, body={body}"),
        },
        _ if status >= 500 => LlmError::ServerError {
            status,
            message: body.to_string(),
        },
        _ => LlmError::BadRequest {
            message: format!("status={status}, body={body}"),
        },
    }
}
