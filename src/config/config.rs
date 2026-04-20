use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct Config {
    inner: Arc<RwLock<ConfigValues>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigValues {
    pub api_key: String,
    pub model: String,
    pub base_url: String,
    pub max_tokens: u32,
    pub thinking_budget: u32,
    pub command_timeout: u64,
    pub max_iterations: usize,
    pub tools: String,
    pub commands: String,
}

impl Default for ConfigValues {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: "claude-sonnet-4-20250514".into(),
            base_url: "https://api.anthropic.com".into(),
            max_tokens: 8096,
            thinking_budget: 0,
            command_timeout: 60,
            max_iterations: 50,
            tools: "all".into(),
            commands: "all".into(),
        }
    }
}

impl Config {
    pub fn new(values: ConfigValues) -> Self {
        Self {
            inner: Arc::new(RwLock::new(values)),
        }
    }

    pub fn get(&self) -> ConfigValues {
        self.inner.read().unwrap().clone()
    }

    #[allow(dead_code)]
    pub fn update(&self, values: ConfigValues) {
        *self.inner.write().unwrap() = values;
    }

    pub fn load(dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(dir)?;
        std::fs::create_dir_all(dir.join("sessions"))?;

        let config_path = dir.join("wgent.json");
        let values = if config_path.exists() {
            let raw = std::fs::read_to_string(&config_path)?;
            serde_json::from_str::<ConfigValues>(&raw)?
        } else {
            let defaults = ConfigValues::default();
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
