use std::sync::{Arc, RwLock};

/// 热加载配置：运行时可通过 update() 修改，各组件下次读取即生效
#[derive(Clone)]
pub struct Config {
    inner: Arc<RwLock<ConfigValues>>,
}

#[derive(Clone, Debug)]
pub struct ConfigValues {
    pub command_timeout: u64,
    pub max_iterations: usize,
    pub max_tokens: u32,
    pub thinking_budget: u32,
}

impl Config {
    pub fn new(values: ConfigValues) -> Self {
        Self {
            inner: Arc::new(RwLock::new(values)),
        }
    }

    /// 获取当前配置快照
    pub fn get(&self) -> ConfigValues {
        self.inner.read().unwrap().clone()
    }

    /// 热更新配置
    #[allow(dead_code)]
    pub fn update(&self, values: ConfigValues) {
        *self.inner.write().unwrap() = values;
    }
}

impl ConfigValues {
    /// 从环境变量加载，缺失项使用默认值
    pub fn from_env() -> Self {
        Self {
            command_timeout: parse_env("AGENT_COMMAND_TIMEOUT", 60),
            max_iterations: parse_env("AGENT_MAX_ITERATIONS", 50),
            max_tokens: parse_env("AGENT_MAX_TOKENS", 8096),
            thinking_budget: parse_env("AGENT_THINKING_BUDGET", 0),
        }
    }
}

fn parse_env<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
