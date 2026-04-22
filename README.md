# Wgent

A Rust-based AI Agent framework powered by the Anthropic Messages API, with tool calling, sub-agent support, session persistence, and an extensible command system.

## Quick Start

```bash
cargo build --release
./target/release/wgent-cli
```

On first run, a configuration file is automatically created at `~/.wgent/wgent.json`. Edit it to set your API key.

## Configuration

Configuration file located at `~/.wgent/wgent.json`, supports hot-reloading:

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

- `tools` / `commands`: Set to `"all"` to enable everything, or comma-separated list (e.g. `"read,write,bash"`)
- API key can also be set via `.env` with `ANTHROPIC_API_KEY`

## Directory Layout

```
~/.wgent/
  wgent.json       Configuration
  sessions/        Session persistence
```

## Architecture

```
cli/               Terminal UI layer (Transport)
src/
  config/          JSON configuration system
  core/            Agent core (self-bootstrapping constructor)
  llm/             LLM provider abstraction
  tools/           Tool registry and built-in tools
  commands/        Slash command system
  prompt/          Tera template prompts
  transport/       Event stream transport
  logging/         Logging initialization
  utils/           Utility functions
```
