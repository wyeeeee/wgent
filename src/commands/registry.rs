use std::collections::HashMap;

use anyhow::{anyhow, Result};

use crate::commands::command::{Command, CommandContext, CommandResult};

pub struct CommandRegistry {
    commands: HashMap<String, Box<dyn Command>>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
        }
    }

    pub fn from_config(spec: &str) -> Self {
        let mut registry = Self::new();
        let names = parse_spec(spec);
        let want_all = names.contains(&"all");

        if want_all || names.contains(&"new") {
            registry.register(Box::new(crate::commands::builtin::NewCommand));
        }
        if want_all || names.contains(&"help") {
            registry.register(Box::new(crate::commands::builtin::HelpCommand));
        }

        registry
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

fn parse_spec(spec: &str) -> Vec<&str> {
    spec.split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect()
}
