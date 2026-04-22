use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Result, bail};
use tokio::sync::RwLock;
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

pub struct Agent {
    llm: Arc<dyn LlmProvider>,
    tools: Arc<RwLock<ToolRegistry>>,
    commands: Arc<RwLock<CommandRegistry>>,
    prompts: Arc<PromptManager>,
    sessions: SessionManager,
    config: Config,
    working_dir: PathBuf,
}

impl Agent {
    pub fn new(dir: &Path, working_dir: &Path) -> Result<Self> {
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
        let tools = ToolRegistry::from_config(&config, &cfg.tools, dir, working_dir);

        let commands = CommandRegistry::from_config(&cfg.commands);
        let sessions = SessionManager::new(dir.join("sessions"));

        Ok(Self {
            llm,
            tools: Arc::new(RwLock::new(tools)),
            commands: Arc::new(RwLock::new(commands)),
            prompts,
            sessions,
            config,
            working_dir: working_dir.to_path_buf(),
        })
    }

    pub(crate) fn new_sub(dir: &Path, working_dir: &Path) -> Result<Self> {
        let config = Config::load(dir)?;
        let cfg = config.get();

        if cfg.api_key.is_empty() {
            bail!("API key not set");
        }

        let llm = Arc::new(crate::llm::AnthropicProvider::new(config.clone()));
        let prompts = Arc::new(PromptManager::new()?);
        let tools = ToolRegistry::from_config_excluding(
            &config, &cfg.tools, &["subagent"], dir, working_dir,
        );

        Ok(Self {
            llm,
            tools: Arc::new(RwLock::new(tools)),
            commands: Arc::new(RwLock::new(CommandRegistry::from_config(&cfg.commands))),
            prompts,
            sessions: SessionManager::new(dir.join("sessions")),
            config,
            working_dir: working_dir.to_path_buf(),
        })
    }

    pub fn session_manager(&self) -> SessionManager {
        self.sessions.clone()
    }

    pub fn commands(&self) -> Arc<RwLock<CommandRegistry>> {
        self.commands.clone()
    }

    pub fn working_dir(&self) -> &Path {
        &self.working_dir
    }

    pub fn model_name(&self) -> String {
        self.config.get().model
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
            run_loop(llm, tools, prompts, config, &mut *guard, &tx).await;
            drop(guard);
            let _ = tx.send(AgentEvent::Done).await;
            let _ = sessions.save(&session).await;
        });

        Ok((sid, rx))
    }
}

async fn run_loop(
    llm: Arc<dyn LlmProvider>,
    tools: Arc<RwLock<ToolRegistry>>,
    prompts: Arc<PromptManager>,
    config: Config,
    session: &mut Session,
    tx: &Sender<AgentEvent>,
) {
    let mut iterations = 0;

    loop {
        iterations += 1;
        let cfg = config.get();
        if iterations > cfg.agent_max_iterations {
            let _ = tx.send(AgentEvent::Error("Maximum iteration limit reached".into())).await;
            return;
        }

        let request = match build_request(session, &prompts, &tools, &cfg).await {
            Ok(r) => r,
            Err(e) => {
                let _ = tx.send(AgentEvent::Error(format!("Failed to build request: {e}"))).await;
                return;
            }
        };

        let response = match llm.chat(request).await {
            Ok(r) => r,
            Err(e) => {
                error!("LLM request failed: {e}");
                let _ = tx.send(AgentEvent::Error(format!("LLM request failed: {e}"))).await;
                return;
            }
        };

        if !process_response(response, session, &tools, &prompts, tx).await {
            return;
        }
    }
}
