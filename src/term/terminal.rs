use std::io::{self, Write};
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use colored::Colorize;
use tracing::debug;

use crate::core::Agent;
use crate::transport::{AgentEvent, Transport};

pub struct TerminalTransport;

impl TerminalTransport {
    pub fn new() -> Self {
        Self
    }

    /// 终端 UI 主循环：首次 chat 自动创建 session，后续自动接续
    pub async fn run(&self, agent: Arc<Agent>, working_dir: &Path) -> Result<()> {
        let mut session_id: Option<String> = None;

        loop {
            let input = self.read_input().await?;
            if input.trim().is_empty() {
                continue;
            }

            let (sid, mut rx) = agent.chat(session_id.as_deref(), &input, working_dir).await?;
            session_id = Some(sid);

            while let Some(event) = rx.recv().await {
                self.send_event(event).await?;
            }
        }
    }
}

#[async_trait]
impl Transport for TerminalTransport {
    async fn read_input(&self) -> Result<String> {
        print!("{}", "\n> ".green().bold());
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        Ok(input.trim_end().to_string())
    }

    async fn send_event(&self, event: AgentEvent) -> Result<()> {
        match event {
            AgentEvent::Thinking(text) => {
                println!("{}", text.dimmed().italic());
            }
            AgentEvent::TextDelta(delta) => {
                print!("{delta}");
                io::stdout().flush()?;
            }
            AgentEvent::TextComplete(text) => {
                println!("{}", text.white());
            }
            AgentEvent::ToolCallStart { name, input_preview, .. } => {
                println!("  {} {}: {}", "⟳".yellow(), format!("{name}").yellow(), input_preview.dimmed());
            }
            AgentEvent::ToolCallEnd { name, result, .. } => {
                debug!("Tool '{name}' result: {result}");
                println!(
                    "  {} {}",
                    "✓".green(),
                    format!("{name}: {result}").dimmed()
                );
            }
            AgentEvent::Error(msg) => {
                println!("{} {}", "✗".red(), msg.red());
            }
            AgentEvent::Done => {
                println!("{}", "─".repeat(40).dimmed());
            }
        }
        Ok(())
    }
}
