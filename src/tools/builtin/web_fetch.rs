use std::time::Duration;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::config::Config;
use crate::tools::tool::{Tool, ToolContext};

pub struct WebFetchTool {
    config: Config,
}

impl WebFetchTool {
    pub fn new(config: Config) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "WebFetch"
    }

    fn description(&self) -> &str {
        "Fetch content from a URL. Returns the response body as text. Only supports HTTP/HTTPS GET requests."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to fetch (HTTP or HTTPS)"
                }
            },
            "required": ["url"]
        })
    }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<String> {
        let url = input["url"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing 'url' parameter"))?;

        if url.trim().is_empty() {
            return Err(anyhow!("URL cannot be empty"));
        }

        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(anyhow!("URL must start with http:// or https://"));
        }

        let cfg = self.config.get();

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(cfg.command_timeout))
            .user_agent("wgent/0.1")
            .build()
            .map_err(|e| anyhow!("Failed to create HTTP client: {e}"))?;

        let resp = client.get(url).send().await.map_err(|e| anyhow!("Request failed: {e}"))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("HTTP {} {}: {}", status.as_u16(), status.canonical_reason().unwrap_or("Error"), truncate(&body, 500)));
        }

        let body = resp.text().await.map_err(|e| anyhow!("Failed to read response body: {e}"))?;

        let max_length = cfg.web_fetch_max_length;
        if body.len() > max_length {
            Ok(format!("{}\n\n[response truncated at {} characters]", truncate(&body, max_length), body.len()))
        } else {
            Ok(body)
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        s[..max].to_string()
    }
}