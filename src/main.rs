mod commands;
mod config;
mod core;
mod llm;
mod prompt;
mod term;
mod tools;
mod transport;
mod utils;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use tracing::info;

use commands::CommandRegistry;
use commands::builtin::{HelpCommand, NewCommand};
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

    let mut commands = CommandRegistry::new();
    commands.register(Box::new(NewCommand));
    commands.register(Box::new(HelpCommand));

    info!("Agent initialized, model={}, working_dir={}", llm.model_name(), working_dir.display());

    let agent = Arc::new(Agent::new(llm, tools, prompts, data_dir, config));

    let transport = TerminalTransport::new(commands);
    transport.run(agent, &working_dir).await
}
