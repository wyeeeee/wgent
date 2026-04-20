use anyhow::Result;
use async_trait::async_trait;

use crate::commands::command::{Command, CommandContext, CommandResult};

pub struct HelpCommand;

#[async_trait]
impl Command for HelpCommand {
    fn name(&self) -> &str {
        "help"
    }

    fn description(&self) -> &str {
        "显示可用命令列表"
    }

    async fn execute(
        &self,
        ctx: &CommandContext,
        _args: Option<&str>,
    ) -> Result<CommandResult> {
        let text = if ctx.command_list.is_empty() {
            "没有可用命令".to_string()
        } else {
            let mut lines = vec!["可用命令:".to_string()];
            for (name, desc) in &ctx.command_list {
                lines.push(format!("  /{:<10} {}", name, desc));
            }
            lines.join("\n")
        };
        Ok(CommandResult::Message { text })
    }
}
