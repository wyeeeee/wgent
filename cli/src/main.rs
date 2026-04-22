use std::io::{self, Write};
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use colored::Colorize;
use tracing::debug;

use wgent::commands::{CommandContext, CommandResult};
use wgent::core::Agent;
use wgent::transport::{AgentEvent, Transport};

struct TerminalTransport;

impl TerminalTransport {
    async fn run(&self, agent: Arc<Agent>) -> Result<()> {
        let mut session_id: Option<String> = None;
        let working_dir = agent.working_dir().to_path_buf();

        loop {
            let input = self.read_input().await?;
            if input.trim().is_empty() {
                continue;
            }

            // Slash command detection
            if let Some(rest) = input.strip_prefix('/') {
                let (cmd_name, args) = match rest.split_once(' ') {
                    Some((name, a)) => (name, Some(a)),
                    None => (rest, None),
                };

                let commands = agent.commands();
                if commands.is_command(cmd_name) {
                    let ctx = CommandContext {
                        session_manager: agent.session_manager(),
                        working_dir: working_dir.clone(),
                        command_list: commands.list()
                            .into_iter()
                            .map(|(n, d)| (n.to_string(), d.to_string()))
                            .collect(),
                    };

                    match commands.execute(cmd_name, &ctx, args).await {
                        Ok(result) => self.render_command_result(result, &mut session_id),
                        Err(e) => println!("{} /{}: {}", "✗".red(), cmd_name, e),
                    }
                    continue;
                }
            }

            // Normal conversation
            let (sid, mut rx) = agent.chat(session_id.as_deref(), &input).await?;
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
            AgentEvent::Done { usage } => {
                println!("{}", "─".repeat(40).dimmed());
                if let Some(u) = usage {
                    println!(
                        "  {} tokens in, {} tokens out",
                        u.input_tokens.to_string().dimmed(),
                        u.output_tokens.to_string().dimmed(),
                    );
                }
            }
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    {
        use tracing_subscriber::EnvFilter;
        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("wgent=info"));
        tracing_subscriber::fmt().with_env_filter(filter).init();
    }

    let data_dir = wgent::config::Config::default_dir();
    let working_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

    let agent = Arc::new(wgent::core::Agent::new(&data_dir, &working_dir)?);

    tracing::info!("Agent initialized, model={}, working_dir={}", agent.model_name(), working_dir.display());

    let transport = TerminalTransport;
    transport.run(agent).await
}
