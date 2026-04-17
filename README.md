# Agent

基于 Rust 的 AI Agent 框架，模块化单 crate 架构，核心 SDK 与 UI 层完全隔离。

## 项目结构

```
src/
├── main.rs              # 入口：组装各模块，UI 驱动主循环
├── core/                # 核心 SDK 层（无 IO 依赖）
│   ├── agent.rs         # Agent SDK：chat(session_id, msg) -> Stream<AgentEvent>
│   ├── session.rs       # Session + SessionManager（内存缓存 + JSON 持久化）
│   ├── message.rs       # 统一消息类型（可序列化）
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

## 架构设计

```
UI 层 (term/web/...) 驱动主循环
  ↓ 调用
Core SDK: agent.chat(session_id, msg) → Receiver<AgentEvent>
  ↓ 内部编排
  LLM (Anthropic) → Tools → Loop → 返回事件流
  ↓ 自动持久化
  SessionManager: 内存缓存 → JSON 文件
```

- Core 是纯 SDK，不知道上层是什么（终端、Web、SDK 调用）
- Session 通过 ID 管理，支持多会话并发
- 上下文内存缓存，每轮结束后持久化到 JSON

## 配置

复制 `.env.example` 为 `.env` 并填入 API Key：

```bash
cp .env.example .env
```

| 环境变量 | 说明 | 默认值 |
|---------|------|-------|
| `ANTHROPIC_API_KEY` | Anthropic API 密钥 | （必填）|
| `ANTHROPIC_MODEL` | 模型名称 | `claude-sonnet-4-20250514` |
| `ANTHROPIC_BASE_URL` | API 基础地址 | `https://api.anthropic.com` |
| `AGENT_DATA_DIR` | Session 数据存储目录 | `data/sessions` |

## 运行

```bash
cargo run
```
