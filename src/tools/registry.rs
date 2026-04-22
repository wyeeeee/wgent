use std::collections::HashMap;
use std::path::Path;

use anyhow::{anyhow, Result};
use serde_json::Value;

use crate::config::Config;
use crate::llm::ToolDefinition;
use crate::tools::tool::{Tool, ToolContext};

pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn from_config(config: &Config, spec: &[String], dir: &Path, working_dir: &Path) -> Self {
        Self::from_config_excluding(config, spec, &[], dir, working_dir)
    }

    pub fn from_config_excluding(
        config: &Config,
        spec: &[String],
        exclude: &[&str],
        dir: &Path,
        working_dir: &Path,
    ) -> Self {
        let mut registry = Self::new();
        let want_all = spec.iter().any(|s| s == "all");

        let all_tools: Vec<(&str, Box<dyn Tool>)> = vec![
            ("Bash", Box::new(crate::tools::builtin::BashTool::new(config.clone()))),
            ("Read", Box::new(crate::tools::builtin::ReadTool)),
            ("Write", Box::new(crate::tools::builtin::WriteTool)),
            ("Edit", Box::new(crate::tools::builtin::EditTool)),
            ("MultiEdit", Box::new(crate::tools::builtin::MultiEditTool)),
            ("Grep", Box::new(crate::tools::builtin::GrepTool::new(config.clone()))),
            ("Ls", Box::new(crate::tools::builtin::LsTool)),
            ("SubAgent", Box::new(crate::tools::builtin::SubAgentTool::new(
                dir.to_path_buf(),
                working_dir.to_path_buf(),
            ))),
        ];

        for (name, tool) in all_tools {
            if (want_all || spec.iter().any(|s| s == name)) && !exclude.contains(&name) {
                registry.register(tool);
            }
        }

        registry
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub async fn execute(&self, name: &str, input: Value, ctx: &ToolContext) -> Result<String> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| anyhow!("Tool not found: {name}"))?;
        tool.execute(input, ctx).await
    }

    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .values()
            .map(|t| ToolDefinition {
                name: t.name().to_string(),
                description: t.description().to_string(),
                input_schema: t.input_schema(),
            })
            .collect()
    }
}
