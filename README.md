# Agent

基于 Rust 的 AI Agent 框架，采用模块化单 crate 架构，核心层与 UI 层完全隔离。

## 项目结构

```
src/
├── main.rs              # 入口：组装各模块，启动 agent
├── core/                # 核心层：agent 循环、消息、会话
│   ├── agent.rs         # Agent 主循环（输入→LLM→工具→输出）
│   ├── message.rs       # 统一消息类型
│   ├── conversation.rs  # 会话状态与历史管理
│   └── error.rs         # 核心错误类型
├── llm/                 # LLM 层：仅 Anthropic，仅 function calling
│   ├── provider.rs      # LlmProvider trait
│   ├── anthropic.rs     # Anthropic Messages API 实现
│   ├── types.rs         # 请求/响应类型与 API 序列化
│   └── error.rs         # LLM 错误类型
├── tools/               # 工具层（当前空置，预留扩展）
│   ├── tool.rs          # Tool trait 定义
│   └── registry.rs      # 工具注册表
├── prompt/              # 提示词层：Jinja2 风格模板
│   ├── manager.rs       # PromptManager 模板引擎
│   └── templates/       # .tera 模板文件
│       ├── system.tera
│       ├── tool_error.tera
│       └── tool_result.tera
├── transport/           # 传输层抽象：UI ↔ Core 桥梁
│   └── transport.rs     # Transport trait + AgentEvent
└── term/                # 终端 UI
    └── terminal.rs      # TerminalTransport 实现
```

## 配置

复制 `.env.example` 为 `.env` 并填入 API Key：

```bash
cp .env.example .env
```

## 运行

```bash
cargo run
```

## 依赖关系

```
main → core → llm (Anthropic)
             → tools (空置)
             → prompt (Tera 模板)
             → transport ← term (终端)
```
