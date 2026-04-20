use std::path::PathBuf;

use anyhow::Result;
use async_trait::async_trait;
use serde::Serialize;

use crate::core::session::SessionManager;

/// 命令执行上下文，按需扩展字段
#[derive(Clone)]
pub struct CommandContext {
    pub session_manager: SessionManager,
    pub working_dir: PathBuf,
}

/// 结构化返回
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(dead_code)]
pub enum CommandResult {
    NewSession { session_id: String },
    Message { text: String },
    Error { message: String },
}

/// 命令抽象
#[async_trait]
pub trait Command: Send + Sync {
    /// 命令名
    fn name(&self) -> &str;

    /// 简短描述（供 /help 使用）
    #[allow(dead_code)]
    fn description(&self) -> &str;

    /// 执行命令
    async fn execute(
        &self,
        ctx: &CommandContext,
        args: Option<&str>,
    ) -> Result<CommandResult>;
}
