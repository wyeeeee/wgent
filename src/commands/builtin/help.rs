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
        "Show available commands"
    }

    async fn execute(
        &self,
        ctx: &CommandContext,
        _args: Option<&str>,
    ) -> Result<CommandResult> {
        let text = if ctx.command_list.is_empty() {
            "No available commands".to_string()
        } else {
            let mut lines = vec!["Available commands:".to_string()];
            for (name, desc) in &ctx.command_list {
                lines.push(format!("  /{:<10} {}", name, desc));
            }
            lines.join("\n")
        };
        Ok(CommandResult::Message { text })
    }
}
