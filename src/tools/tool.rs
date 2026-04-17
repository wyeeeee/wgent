use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use std::path::Path;

/// 工具抽象 trait
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> Value;
    async fn execute(&self, input: Value, working_dir: &Path) -> Result<String>;
}
