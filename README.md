# Wgent

基于 Rust 的 AI Agent 框架。

## 快速开始

```bash
cargo build --release
./target/release/wgent-cli
```

首次运行自动在 `~/.wgent/` 创建 `wgent.json` 配置文件，编辑填入 API key 即可使用。

## 配置

配置文件位于 `~/.wgent/wgent.json`，支持热加载：

```json
{
  "api_key": "",
  "model": "claude-sonnet-4-20250514",
  "base_url": "https://api.anthropic.com",
  "max_tokens": 8096,
  "thinking_budget": 0,
  "command_timeout": 60,
  "agent_max_iterations": 50,
  "llm_max_retries": 10,
  "grep_max_results": 50,
  "tools": "all",
  "commands": "all"
}
```

- `tools` / `commands`：`"all"` 启用全部，或逗号分隔指定（如 `"read,write,bash"`）
- 也可通过 `.env` 设置 `ANTHROPIC_API_KEY`

## 目录结构

```
~/.wgent/
  wgent.json       配置
  sessions/        会话持久化
```

## 架构

```
cli/               终端 UI 层（Transport）
src/
  config/          JSON 配置系统
  core/            Agent 核心（自举构造）
  llm/             LLM Provider 抽象
  tools/           工具注册与内置工具
  commands/        Slash 命令
  prompt/          Tera 模板提示词
  transport/       事件流传输
  utils/           工具函数
```
