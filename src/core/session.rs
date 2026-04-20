use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, warn};

use crate::core::message::Message;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub working_dir: PathBuf,
    pub created_at: i64,
    pub updated_at: i64,
    pub messages: Vec<Message>,
}

impl Session {
    pub fn new(id: String, working_dir: PathBuf) -> Self {
        let now = now_ts();
        Self {
            id,
            working_dir,
            created_at: now,
            updated_at: now,
            messages: Vec::new(),
        }
    }

    pub fn add_message(&mut self, message: Message) {
        self.updated_at = now_ts();
        self.messages.push(message);
    }
}

fn now_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

// ── SessionManager ──

#[derive(Clone)]
pub struct SessionManager {
    inner: Arc<SessionManagerInner>,
}

struct SessionManagerInner {
    cache: RwLock<HashMap<String, Session>>,
    data_dir: PathBuf,
}

impl SessionManager {
    pub fn new(data_dir: PathBuf) -> Self {
        std::fs::create_dir_all(&data_dir).ok();
        Self {
            inner: Arc::new(SessionManagerInner {
                cache: RwLock::new(HashMap::new()),
                data_dir,
            }),
        }
    }

    /// 获取或创建 session（先查缓存，再查文件，最后新建）
    pub async fn get_or_create(&self, id: &str, working_dir: PathBuf) -> Result<Session> {
        {
            let cache = self.inner.cache.read().await;
            if let Some(session) = cache.get(id) {
                return Ok(session.clone());
            }
        }

        let path = self.file_path(id);
        if path.exists() {
            let data = tokio::fs::read_to_string(&path).await?;
            match serde_json::from_str::<Session>(&data) {
                Ok(session) => {
                    let mut cache = self.inner.cache.write().await;
                    cache.insert(id.to_string(), session.clone());
                    debug!("session loaded from file: {id}");
                    return Ok(session);
                }
                Err(e) => {
                    warn!("session file corrupted {id}: {e}, creating new");
                }
            }
        }

        let session = Session::new(id.to_string(), working_dir);
        let mut cache = self.inner.cache.write().await;
        cache.insert(id.to_string(), session.clone());
        debug!("session created: {id}");
        Ok(session)
    }

    /// 持久化 session（更新缓存 + 写文件）
    pub async fn save(&self, session: &Session) -> Result<()> {
        {
            let mut cache = self.inner.cache.write().await;
            cache.insert(session.id.clone(), session.clone());
        }

        let path = self.file_path(&session.id);
        let data = serde_json::to_string_pretty(session)?;
        tokio::fs::write(&path, data).await?;
        debug!("session saved: {}", session.id);
        Ok(())
    }

    fn file_path(&self, id: &str) -> PathBuf {
        let safe = id.replace(['/', '\\', ':'], "_");
        self.inner.data_dir.join(format!("{safe}.json"))
    }
}
