use anyhow::Result;
use tokio::sync::RwLock;

use crate::config::ConfigValues;
use crate::core::message::MessageContent;
use crate::core::session::Session;
use crate::llm::types::*;
use crate::prompt::PromptManager;
use crate::tools::ToolRegistry;

pub async fn build_request(
    session: &Session,
    prompts: &PromptManager,
    tools: &RwLock<ToolRegistry>,
    cfg: &ConfigValues,
) -> Result<ChatRequest> {
    let system = prompts.render_system("Wgent", None::<&str>, &[], &session.working_dir)?;
    let tool_defs = tools.read().await.definitions();

    let messages = session
        .messages
        .iter()
        .map(|msg| ChatMessage {
            role: msg.role.clone(),
            content: msg
                .content
                .iter()
                .map(|c| match c {
                    MessageContent::Thinking { text } => ContentBlock::Thinking { text: text.clone() },
                    MessageContent::Text { text } => ContentBlock::Text { text: text.clone() },
                    MessageContent::ToolCall { id, name, arguments } => ContentBlock::ToolUse {
                        id: id.clone(),
                        name: name.clone(),
                        input: arguments.clone(),
                    },
                    MessageContent::ToolResult { tool_use_id, output } => {
                        ContentBlock::ToolResult {
                            tool_use_id: tool_use_id.clone(),
                            content: output.clone(),
                        }
                    }
                })
                .collect(),
        })
        .collect();

    Ok(ChatRequest {
        model: String::new(),
        max_tokens: cfg.max_tokens,
        system: Some(system),
        messages,
        tools: tool_defs,
        thinking_budget: cfg.thinking_budget,
    })
}
