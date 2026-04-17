mod core;
mod llm;
mod prompt;
mod term;
mod tools;
mod transport;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

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
    let base_url = std::env::var("ANTHROPIC_BASE_URL")
        .unwrap_or_else(|_| "https://api.anthropic.com".to_string());
    let data_dir = std::env::var("AGENT_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("data/sessions"));

    let llm = Arc::new(AnthropicProvider::with_base_url(api_key, model, base_url));
    let prompts = Arc::new(PromptManager::new()?);
    let tools = ToolRegistry::new();

    info!("Agent initialized, model={}", llm.model_name());

    let agent = Arc::new(Agent::new(llm, tools, prompts, data_dir));
    let session_id = generate_session_id();
    info!("session: {session_id}");

    let transport = TerminalTransport::new();
    transport.run(agent, &session_id).await
}

fn generate_session_id() -> String {
    let ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let hash = fnv1a(&ns.to_le_bytes());
    format!("{hash:08x}")
}

fn fnv1a(data: &[u8]) -> u32 {
    let mut hash: u32 = 0x811c9dc5;
    for &b in data {
        hash ^= b as u32;
        hash = hash.wrapping_mul(0x01000193);
    }
    hash
}
