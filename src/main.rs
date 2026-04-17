mod core;
mod llm;
mod prompt;
mod term;
mod tools;
mod transport;

use std::sync::Arc;

use anyhow::Result;
use tracing::info;

use core::Agent;
use llm::AnthropicProvider;
use prompt::PromptManager;
use term::TerminalTransport;
use tools::ToolRegistry;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("agent=info".parse()?),
        )
        .init();

    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .expect("请设置 ANTHROPIC_API_KEY 环境变量");
    let model = std::env::var("ANTHROPIC_MODEL")
        .unwrap_or_else(|_| "claude-sonnet-4-20250514".to_string());

    let llm = Arc::new(AnthropicProvider::new(api_key, model));
    let transport = Arc::new(TerminalTransport::new());
    let prompts = Arc::new(PromptManager::new()?);
    let tools = ToolRegistry::new();

    info!("Agent initialized, model={}", llm.model_name());

    let mut agent = Agent::new(llm, tools, transport, prompts, 100);
    agent.run().await?;

    Ok(())
}
