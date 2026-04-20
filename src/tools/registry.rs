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

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn from_config(config: &Config, spec: &str, dir: &Path, working_dir: &Path) -> Self {
        let mut registry = Self::new();
        let names = parse_spec(spec);
        let want_all = names.contains(&"all");

        if want_all || names.contains(&"bash") {
            registry.register(Box::new(crate::tools::builtin::BashTool::new(config.clone())));
        }
        if want_all || names.contains(&"read") {
            registry.register(Box::new(crate::tools::builtin::ReadTool));
        }
        if want_all || names.contains(&"write") {
            registry.register(Box::new(crate::tools::builtin::WriteTool));
        }
        if want_all || names.contains(&"edit") {
            registry.register(Box::new(crate::tools::builtin::EditTool));
        }
        if want_all || names.contains(&"grep") {
            registry.register(Box::new(crate::tools::builtin::GrepTool::new(config.clone())));
        }
        if want_all || names.contains(&"subagent") {
            registry.register(Box::new(crate::tools::builtin::SubAgentTool::new(
                dir.to_path_buf(),
                working_dir.to_path_buf(),
            )));
        }

        registry
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    #[allow(dead_code)]
    pub fn get(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(|t| t.as_ref())
    }

    pub async fn execute(&self, name: &str, input: Value, ctx: &ToolContext) -> Result<String> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| anyhow!("tool not found: {name}"))?;
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

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    pub fn remove(&mut self, name: &str) {
        self.tools.remove(name);
    }
}

fn parse_spec(spec: &str) -> Vec<&str> {
    spec.split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect()
}
