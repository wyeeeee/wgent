use serde::{Deserialize, Serialize};

use crate::llm::types::Role;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageContent {
    Thinking { text: String },
    Text { text: String },
    ToolCall { id: String, name: String, arguments: serde_json::Value },
    ToolResult { tool_use_id: String, output: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: Vec<MessageContent>,
}

impl Message {
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: vec![MessageContent::Text { text: text.into() }],
        }
    }
}
