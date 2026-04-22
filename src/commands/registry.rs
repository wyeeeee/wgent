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

    pub fn from_config(spec: &[String]) -> Self {
        let mut registry = Self::new();
        let want_all = spec.iter().any(|s| s == "all");

        let all_commands: Vec<(&str, Box<dyn Command>)> = vec![
            ("help", Box::new(crate::commands::builtin::HelpCommand)),
            ("new", Box::new(crate::commands::builtin::NewCommand)),
        ];

        for (name, cmd) in all_commands {
            if want_all || spec.iter().any(|s| s == name) {
                registry.register(cmd);
            }
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
            .ok_or_else(|| anyhow!("Unknown command: /{name}"))?;
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
