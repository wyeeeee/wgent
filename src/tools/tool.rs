use std::path::PathBuf;

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::mpsc::Sender;

use crate::transport::AgentEvent;

pub struct ToolContext {
    pub working_dir: PathBuf,
    pub events: Option<Sender<AgentEvent>>,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> Value;
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String>;
}
