use std::io::{self, Write};
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use colored::Colorize;
use tracing::debug;

use crate::commands::{CommandContext, CommandRegistry, CommandResult};
use crate::core::Agent;
use crate::transport::{AgentEvent, Transport};

pub struct TerminalTransport {
    commands: CommandRegistry,
}

impl TerminalTransport {
    pub fn new(commands: CommandRegistry) -> Self {
        Self { commands }
    }

    /// 终端 UI 主循环：首次 chat 自动创建 session，后续自动接续
    pub async fn run(&self, agent: Arc<Agent>, working_dir: &Path) -> Result<()> {
        let mut session_id: Option<String> = None;

        loop {
            let input = self.read_input().await?;
            if input.trim().is_empty() {
                continue;
            }

            // Slash 命令检测
            if let Some(rest) = input.strip_prefix('/') {
                let (cmd_name, args) = match rest.split_once(' ') {
                    Some((name, a)) => (name, Some(a)),
                    None => (rest, None),
                };

                if self.commands.is_command(cmd_name) {
                    let ctx = CommandContext {
                        session_manager: agent.session_manager(),
                        working_dir: working_dir.to_path_buf(),
                        command_list: self.commands.list()
                            .into_iter()
                            .map(|(n, d)| (n.to_string(), d.to_string()))
                            .collect(),
                    };

                    match self.commands.execute(cmd_name, &ctx, args).await {
                        Ok(result) => self.render_command_result(result, &mut session_id),
                        Err(e) => println!("{} /{}: {}", "✗".red(), cmd_name, e),
                    }
                    continue;
                }
            }

            // 普通对话
            let (sid, mut rx) = agent.chat(session_id.as_deref(), &input, working_dir).await?;
            session_id = Some(sid);

            while let Some(event) = rx.recv().await {
                self.send_event(event).await?;
            }
        }
    }

    fn render_command_result(&self, result: CommandResult, session_id: &mut Option<String>) {
        match result {
            CommandResult::NewSession { session_id: new_id } => {
                *session_id = Some(new_id.clone());
                println!("{} New session: {}", "✓".green(), new_id.yellow());
            }
            CommandResult::Message { text } => {
                println!("{}", text);
            }
            CommandResult::Error { message } => {
                println!("{} {}", "✗".red(), message.red());
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
