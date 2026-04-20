use anyhow::Result;
use async_trait::async_trait;

use crate::commands::command::{Command, CommandContext, CommandResult};

pub struct NewCommand;

#[async_trait]
impl Command for NewCommand {
    fn name(&self) -> &str {
        "new"
    }

    fn description(&self) -> &str {
        "开始新的会话"
    }

    async fn execute(
        &self,
        ctx: &CommandContext,
        _args: Option<&str>,
    ) -> Result<CommandResult> {
        let new_id = ctx.session_manager.generate_id();
        ctx.session_manager
            .get_or_create(&new_id, ctx.working_dir.clone())
            .await?;
        Ok(CommandResult::NewSession { session_id: new_id })
    }
}
