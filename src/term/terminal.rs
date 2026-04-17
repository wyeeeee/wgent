use std::io::{self, Write};

use anyhow::Result;
use async_trait::async_trait;
use colored::Colorize;
use tracing::debug;

use crate::transport::{AgentEvent, Transport};

pub struct TerminalTransport;

impl TerminalTransport {
    pub fn new() -> Self {
        Self
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
            AgentEvent::ToolCallStart { name, .. } => {
                println!("  {} {}", "⟳".yellow(), format!("调用工具: {name}").yellow());
            }
            AgentEvent::ToolCallEnd { name, result, .. } => {
                let preview = if result.len() > 200 {
                    format!("{}...", &result[..200])
                } else {
                    result
                };
                debug!("Tool '{name}' result: {preview}");
                println!(
                    "  {} {}",
                    "✓".green(),
                    format!("{name}: {preview}").dimmed()
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
