use crate::llm::types::Role;

/// 统一消息内容块（内部表示）
#[derive(Clone, Debug)]
pub enum MessageContent {
    Text(String),
    ToolCall { id: String, name: String, arguments: serde_json::Value },
    ToolResult { tool_use_id: String, output: String },
}

/// 统一消息（内部表示）
#[derive(Clone, Debug)]
pub struct Message {
    pub role: Role,
    pub content: Vec<MessageContent>,
}

impl Message {
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: vec![MessageContent::Text(text.into())],
        }
    }

    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: vec![MessageContent::Text(text.into())],
        }
    }
}
