use anyhow::Result;
use async_trait::async_trait;

/// Agent → UI 的事件流
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum AgentEvent {
    /// 模型正在思考（可选展示）
    Thinking(String),
    /// 文本流增量
    TextDelta(String),
    /// 完整文本响应
    TextComplete(String),
    /// 工具调用开始
    ToolCallStart { id: String, name: String },
    /// 工具调用结束
    ToolCallEnd { id: String, name: String, result: String },
    /// 错误信息
    Error(String),
    /// 本轮对话结束
    Done,
}

/// 传输层抽象：UI ↔ Core 的桥梁
#[async_trait]
pub trait Transport: Send + Sync {
    /// 阻塞读取用户输入
    async fn read_input(&self) -> Result<String>;
    /// 向 UI 推送 agent 事件
    async fn send_event(&self, event: AgentEvent) -> Result<()>;
}
