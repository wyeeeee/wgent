use std::path::PathBuf;

use anyhow::Result;
use async_trait::async_trait;
use serde::Serialize;

use crate::core::session::SessionManager;

/// Command execution context
#[derive(Clone)]
pub struct CommandContext {
    pub session_manager: SessionManager,
    pub working_dir: PathBuf,
    pub command_list: Vec<(String, String)>,
}

/// Structured command result
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(dead_code)]
pub enum CommandResult {
    NewSession { session_id: String },
    Message { text: String },
    Error { message: String },
}

/// Command abstraction
#[async_trait]
pub trait Command: Send + Sync {
    /// Command name
    fn name(&self) -> &str;

    /// Short description (for /help)
    #[allow(dead_code)]
    fn description(&self) -> &str;

    /// Execute the command
    async fn execute(
        &self,
        ctx: &CommandContext,
        args: Option<&str>,
    ) -> Result<CommandResult>;
}
