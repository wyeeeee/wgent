use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct Config {
    inner: Arc<ConfigValues>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigValues {
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_base_url")]
    pub base_url: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default)]
    pub thinking_budget: u32,
    #[serde(default = "default_command_timeout")]
    pub command_timeout: u64,
    #[serde(default = "default_agent_max_iterations")]
    pub agent_max_iterations: usize,
    #[serde(default = "default_llm_max_retries")]
    pub llm_max_retries: usize,
    #[serde(default = "default_grep_max_results")]
    pub grep_max_results: usize,
    #[serde(default = "default_web_fetch_max_length")]
    pub web_fetch_max_length: usize,
    #[serde(default = "default_tools")]
    pub tools: Vec<String>,
    #[serde(default = "default_commands")]
    pub commands: Vec<String>,
}

fn default_model() -> String { "claude-sonnet-4-20250514".into() }
fn default_base_url() -> String { "https://api.anthropic.com".into() }
fn default_max_tokens() -> u32 { 120_000 }
fn default_command_timeout() -> u64 { 60 }
fn default_agent_max_iterations() -> usize { 50 }
fn default_llm_max_retries() -> usize { 10 }
fn default_grep_max_results() -> usize { 50 }
fn default_web_fetch_max_length() -> usize { 500_000 }
fn default_tools() -> Vec<String> { vec!["all".into()] }
fn default_commands() -> Vec<String> { vec!["all".into()] }

impl Config {
    pub fn new(values: ConfigValues) -> Self {
        Self {
            inner: Arc::new(values),
        }
    }

    pub fn get(&self) -> &ConfigValues {
        &self.inner
    }

    pub fn load(dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(dir)?;
        std::fs::create_dir_all(dir.join("sessions"))?;

        let config_path = dir.join("wgent.json");
        let values = if config_path.exists() {
            let raw = std::fs::read_to_string(&config_path)?;
            serde_json::from_str::<ConfigValues>(&raw)?
        } else {
            let defaults: ConfigValues = serde_json::from_str("{}")?;
            let json = serde_json::to_string_pretty(&defaults)?;
            std::fs::write(&config_path, json)?;
            defaults
        };

        Ok(Self::new(values))
    }

    pub fn default_dir() -> PathBuf {
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        home.join(".wgent")
    }
}
