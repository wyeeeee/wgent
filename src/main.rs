mod config;
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

use config::{Config, ConfigValues};
use core::Agent;
use llm::AnthropicProvider;
use prompt::PromptManager;
use term::TerminalTransport;
use tools::builtin::{BashTool, EditTool, ReadTool, WriteTool};
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

    let config = Config::new(ConfigValues::from_env());
    let data_dir = std::env::var("AGENT_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("data/sessions"));
    let working_dir = std::env::var("AGENT_WORKING_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let llm = Arc::new(AnthropicProvider::new(config.clone()));
    let prompts = Arc::new(PromptManager::new()?);

    let mut tools = ToolRegistry::new();
    tools.register(Box::new(ReadTool));
    tools.register(Box::new(WriteTool));
    tools.register(Box::new(EditTool));
    tools.register(Box::new(BashTool::new(config.clone())));

    info!("Agent initialized, model={}, working_dir={}", llm.model_name(), working_dir.display());

    let agent = Arc::new(Agent::new(llm, tools, prompts, data_dir, config));
    let session_id = generate_session_id();
    info!("session: {session_id}");

    let transport = TerminalTransport::new();
    transport.run(agent, &session_id, &working_dir).await
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
