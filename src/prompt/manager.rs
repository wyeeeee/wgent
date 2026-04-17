use anyhow::Result;
use tera::{Context, Tera};

const SYSTEM_TEMPLATE: &str = include_str!("templates/system.tera");
const TOOL_ERROR_TEMPLATE: &str = include_str!("templates/tool_error.tera");
const TOOL_RESULT_TEMPLATE: &str = include_str!("templates/tool_result.tera");

pub struct PromptManager {
    tera: Tera,
}

impl PromptManager {
    pub fn new() -> Result<Self> {
        let mut tera = Tera::default();
        tera.add_raw_template("system", SYSTEM_TEMPLATE)?;
        tera.add_raw_template("tool_error", TOOL_ERROR_TEMPLATE)?;
        tera.add_raw_template("tool_result", TOOL_RESULT_TEMPLATE)?;
        Ok(Self { tera })
    }

    pub fn render(&self, name: &str, context: &Context) -> Result<String> {
        Ok(self.tera.render(name, context)?)
    }

    pub fn render_system(
        &self,
        agent_name: &str,
        role: Option<&str>,
        guidelines: &[String],
    ) -> Result<String> {
        let mut ctx = Context::new();
        ctx.insert("agent_name", agent_name);
        ctx.insert("role", &role);
        ctx.insert("guidelines", guidelines);
        self.render("system", &ctx)
    }

    pub fn render_tool_error(&self, tool_name: &str, error: &str) -> Result<String> {
        let mut ctx = Context::new();
        ctx.insert("tool_name", tool_name);
        ctx.insert("error", error);
        self.render("tool_error", &ctx)
    }

    pub fn render_tool_result(&self, tool_name: &str, result: &str) -> Result<String> {
        let mut ctx = Context::new();
        ctx.insert("tool_name", tool_name);
        ctx.insert("result", result);
        self.render("tool_result", &ctx)
    }
}
