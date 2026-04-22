# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Wgent is a Rust-based AI Agent framework using the Anthropic Messages API, supporting tool calling, sub-agents, session persistence, and an extensible command system.

## Build & Run

```bash
cargo build                    # Build workspace
cargo build --release          # Release build
cargo run -p wgent-cli         # Run CLI
cargo check                    # Fast type check
cargo test                     # Run tests
cargo test -p wgent            # Library tests only
cargo test -p wgent-cli        # CLI tests only
```

Rust edition 2024, workspace with `wgent` (library) and `wgent-cli` (binary) crates.

## Architecture

Core data flow: **User input → Agent → LLM API → Response parsing → Parallel tool execution → Event stream output**

Key layers:

- **`core/agent`** — Agent is the central hub, holding LlmProvider / ToolRegistry / CommandRegistry / PromptManager / SessionManager, sending `AgentEvent` via `mpsc` channel. Core loop in `run_loop()`: build request → send to LLM → process response (with parallel tool calls) → write session.
- **`llm/`** — `LlmProvider` trait abstracts LLM calls, `AnthropicProvider` implements retry logic. `types.rs` defines bidirectional conversions between internal types and Anthropic API serialization types.
- **`tools/`** — `Tool` trait (name / description / input_schema / execute), `ToolRegistry` dynamically registers based on config string. Built-in tools: bash, read, write, edit, grep, subagent (non-recursive).
- **`commands/`** — `Command` trait, a slash command system parallel to tools (`/help`, `/new`), managed through `CommandRegistry`.
- **`transport/`** — `Transport` trait + `AgentEvent` enum, defining the event stream protocol between Agent and UI.
- **`config/`** — JSON config hot-reloading (`Arc<RwLock<ConfigValues>>`), auto-generates `~/.wgent/wgent.json` on first run.
- **`core/session`** — Session persistence to `~/.wgent/sessions/`, in-memory cache + file storage, `Arc<RwLock<Session>>` for concurrency safety.
- **`prompt/`** — Tera template engine, system prompt and tool_error templates embedded via `include_str!`.
- **`logging/`** — Tracing subscriber initialization, called by the binary to set up structured logging.

## Design Patterns

- Tools/commands are registered via trait + Registry pattern, config string `"all"` or comma-separated enables them.
- Sub-agent (SubAgentTool) creates an independent Agent instance (excluding itself to prevent recursion), reusing the same config.
- Tool calls execute in parallel via `tokio::spawn` during response processing.
- CLI layer (`cli/`) implements `Transport` trait, is a pure UI layer with no business logic.

## Configuration

`~/.wgent/wgent.json` controls all runtime parameters. `tools` and `commands` fields support `"all"` or comma-separated values.
