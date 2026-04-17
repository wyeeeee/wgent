use serde::{Deserialize, Serialize};

// ── 请求类型 ──

#[derive(Clone, Debug)]
pub struct ChatRequest {
    pub model: String,
    pub max_tokens: u32,
    pub system: Option<String>,
    pub messages: Vec<ChatMessage>,
    pub tools: Vec<ToolDefinition>,
}

#[derive(Clone, Debug)]
pub struct ChatMessage {
    pub role: Role,
    pub content: Vec<ContentBlock>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

#[derive(Clone, Debug)]
pub enum ContentBlock {
    Text { text: String },
    ToolUse { id: String, name: String, input: serde_json::Value },
    ToolResult { tool_use_id: String, content: String },
}

#[derive(Clone, Debug)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

// ── 响应类型 ──

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct ChatResponse {
    pub id: String,
    pub model: String,
    pub content: Vec<ContentBlock>,
    pub stop_reason: StopReason,
    pub usage: Usage,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
}

#[derive(Clone, Debug)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

// ── Anthropic API 序列化结构 ──

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) struct ApiRequest {
    model: String,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<ApiMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<ApiTool>,
}

#[derive(Serialize)]
pub(super) struct ApiMessage {
    role: String,
    content: Vec<ApiContentBlock>,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum ApiContentBlock {
    Text { text: String },
    ToolUse { id: String, name: String, input: serde_json::Value },
    ToolResult { tool_use_id: String, content: String },
}

#[derive(Serialize)]
pub(super) struct ApiTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

#[derive(Deserialize)]
pub struct ApiResponse {
    id: String,
    model: String,
    content: Vec<ApiContentBlockResp>,
    stop_reason: String,
    usage: ApiUsage,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
pub(super) enum ApiContentBlockResp {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse { id: String, name: String, input: serde_json::Value },
}

#[derive(Deserialize)]
pub(super) struct ApiUsage {
    input_tokens: u32,
    output_tokens: u32,
}

// ── 转换实现 ──

impl ChatRequest {
    pub(super) fn to_api(&self) -> ApiRequest {
        ApiRequest {
            model: self.model.clone(),
            max_tokens: self.max_tokens,
            system: self.system.clone(),
            messages: self.messages.iter().map(|m| m.to_api()).collect(),
            tools: self.tools.iter().map(|t| t.to_api()).collect(),
        }
    }
}

impl ChatMessage {
    pub(super) fn to_api(&self) -> ApiMessage {
        let role = match self.role {
            Role::User => "user",
            Role::Assistant => "assistant",
        };
        ApiMessage {
            role: role.to_string(),
            content: self.content.iter().map(|b| b.to_api()).collect(),
        }
    }
}

impl ContentBlock {
    pub(super) fn to_api(&self) -> ApiContentBlock {
        match self {
            Self::Text { text } => ApiContentBlock::Text { text: text.clone() },
            Self::ToolUse { id, name, input } => ApiContentBlock::ToolUse {
                id: id.clone(),
                name: name.clone(),
                input: input.clone(),
            },
            Self::ToolResult { tool_use_id, content } => ApiContentBlock::ToolResult {
                tool_use_id: tool_use_id.clone(),
                content: content.clone(),
            },
        }
    }
}

impl ToolDefinition {
    pub(super) fn to_api(&self) -> ApiTool {
        ApiTool {
            name: self.name.clone(),
            description: self.description.clone(),
            input_schema: self.input_schema.clone(),
        }
    }
}

impl TryFrom<ApiResponse> for ChatResponse {
    type Error = String;

    fn try_from(resp: ApiResponse) -> Result<Self, Self::Error> {
        let stop_reason = match resp.stop_reason.as_str() {
            "end_turn" => StopReason::EndTurn,
            "tool_use" => StopReason::ToolUse,
            "max_tokens" => StopReason::MaxTokens,
            other => return Err(format!("unknown stop_reason: {other}")),
        };

        let content = resp
            .content
            .into_iter()
            .map(|block| match block {
                ApiContentBlockResp::Text { text } => ContentBlock::Text { text },
                ApiContentBlockResp::ToolUse { id, name, input } => {
                    ContentBlock::ToolUse { id, name, input }
                }
            })
            .collect();

        Ok(ChatResponse {
            id: resp.id,
            model: resp.model,
            content,
            stop_reason,
            usage: Usage {
                input_tokens: resp.usage.input_tokens,
                output_tokens: resp.usage.output_tokens,
            },
        })
    }
}
