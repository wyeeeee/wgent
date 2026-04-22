use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Result, bail};
use tokio::sync::mpsc::{Receiver, Sender};
use tracing::error;

use crate::commands::CommandRegistry;
use crate::config::Config;
use crate::core::message::Message;
use crate::core::request::build_request;
use crate::core::response::process_response;
use crate::core::session::{Session, SessionManager};
use crate::llm::provider::LlmProvider;
use crate::prompt::PromptManager;
use crate::tools::ToolRegistry;
use crate::transport::AgentEvent;
use crate::transport::TokenUsage;

pub struct Agent {
    llm: Arc<dyn LlmProvider>,
    tools: Arc<ToolRegistry>,
    commands: Arc<CommandRegistry>,
    prompts: Arc<PromptManager>,
    sessions: SessionManager,
    config: Config,
    working_dir: PathBuf,
}

impl Agent {
    pub fn new(dir: &Path, working_dir: &Path) -> Result<Self> {
        Self::build(dir, working_dir, &[])
    }

    pub(crate) fn new_sub(dir: &Path, working_dir: &Path) -> Result<Self> {
        Self::build(dir, working_dir, &["SubAgent"])
    }

    fn build(dir: &Path, working_dir: &Path, exclude_tools: &[&str]) -> Result<Self> {
        let config = Config::load(dir)?;
        let cfg = config.get();

        if cfg.api_key.is_empty() {
            bail!(
                "API key not set, please edit {}/wgent.json",
                dir.display()
            );
        }

        let llm = Arc::new(crate::llm::AnthropicProvider::new(config.clone()));
        let prompts = Arc::new(PromptManager::new()?);
        let tools = Arc::new(ToolRegistry::from_config_excluding(&config, &cfg.tools, exclude_tools, dir, working_dir));
        let commands = Arc::new(CommandRegistry::from_config(&cfg.commands));

        Ok(Self {
            llm,
            tools,
            commands,
            prompts,
            sessions: SessionManager::new(dir.join("sessions")),
            config,
            working_dir: working_dir.to_path_buf(),
        })
    }

    pub fn session_manager(&self) -> SessionManager {
        self.sessions.clone()
    }

    pub fn commands(&self) -> Arc<CommandRegistry> {
        self.commands.clone()
    }

    pub fn working_dir(&self) -> &Path {
        &self.working_dir
    }

    pub fn model_name(&self) -> String {
        self.config.get().model.clone()
    }

    pub async fn chat(
        &self,
        session_id: Option<&str>,
        user_message: &str,
    ) -> Result<(String, Receiver<AgentEvent>)> {
        let (tx, rx) = tokio::sync::mpsc::channel(32);

        let sid = match session_id {
            Some(id) => id.to_string(),
            None => self.sessions.generate_id(),
        };

        let session = self
            .sessions
            .get_or_create(&sid, self.working_dir.to_path_buf())
            .await?;

        let mut guard = session.clone().write_owned().await;
        guard.add_message(Message::user(user_message));

        let llm = self.llm.clone();
        let tools = self.tools.clone();
        let prompts = self.prompts.clone();
        let config = self.config.clone();
        let sessions = self.sessions.clone();

        tokio::spawn(async move {
            let usage = run_loop(llm, tools, prompts, config, &mut guard, &tx).await;
            drop(guard);
            if tx.send(AgentEvent::Done { usage: Some(usage) }).await.is_err() {
                error!("Failed to send Done event: channel closed");
            }
            if let Err(e) = sessions.save(&session).await {
                error!("Failed to save session: {e}");
            }
        });

        Ok((sid, rx))
    }
}

async fn run_loop(
    llm: Arc<dyn LlmProvider>,
    tools: Arc<ToolRegistry>,
    prompts: Arc<PromptManager>,
    config: Config,
    session: &mut Session,
    tx: &Sender<AgentEvent>,
) -> TokenUsage {
    let mut iterations = 0;
    let mut total_usage = TokenUsage::default();

    loop {
        iterations += 1;
        let cfg = config.get();
        if iterations > cfg.agent_max_iterations {
            error!("Maximum iteration limit reached");
            return total_usage;
        }

        let request = match build_request(session, &prompts, &tools, cfg).await {
            Ok(r) => r,
            Err(e) => {
                error!("Failed to build request: {e}");
                return total_usage;
            }
        };

        let response = match llm.chat(request).await {
            Ok(r) => r,
            Err(e) => {
                error!("LLM request failed: {e}");
                return total_usage;
            }
        };

        total_usage.accumulate(response.usage.input_tokens, response.usage.output_tokens);

        if !process_response(response, session, &tools, &prompts, tx).await {
            return total_usage;
        }
    }
}
