use std::collections::HashMap;

use anyhow::{anyhow, Result};

use crate::commands::command::{Command, CommandContext, CommandResult};

/// 命令注册表
pub struct CommandRegistry {
    commands: HashMap<String, Box<dyn Command>>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
        }
    }

    pub fn register(&mut self, cmd: Box<dyn Command>) {
        self.commands.insert(cmd.name().to_string(), cmd);
    }

    pub fn is_command(&self, name: &str) -> bool {
        self.commands.contains_key(name)
    }

    pub async fn execute(
        &self,
        name: &str,
        ctx: &CommandContext,
        args: Option<&str>,
    ) -> Result<CommandResult> {
        let cmd = self
            .commands
            .get(name)
            .ok_or_else(|| anyhow!("未知命令: /{name}"))?;
        cmd.execute(ctx, args).await
    }

    #[allow(dead_code)]
    pub fn list(&self) -> Vec<(&str, &str)> {
        let mut list: Vec<_> = self
            .commands
            .values()
            .map(|c| (c.name(), c.description()))
            .collect();
        list.sort_by_key(|(name, _)| *name);
        list
    }
}
