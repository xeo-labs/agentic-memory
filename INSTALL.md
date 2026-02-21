# Installation Guide

## Quick Install (one-liner)

```bash
curl -fsSL https://raw.githubusercontent.com/agentralabs/agentic-memory/main/scripts/install.sh | bash
```

Downloads a pre-built `agentic-memory-mcp` binary, installs to `~/.local/bin/`, and merges the MCP server config into Claude Desktop and Claude Code. Memory defaults to `~/.brain.amem`. Requires `curl` and `jq`.

---

## 1. Python SDK (recommended for most users)

The Python SDK gives you the `Brain` class and LLM integrations. Requires **Python 3.10+**.

```bash
pip install agentic-brain
```

### With LLM provider integrations

```bash
pip install agentic-brain[anthropic]   # Claude
pip install agentic-brain[openai]      # GPT
pip install agentic-brain[ollama]      # Local models
pip install agentic-brain[all]         # All providers
```

### Verify

```python
from agentic_memory import Brain

brain = Brain("test.amem")
brain.add_fact("Installation successful", session=1)
print(brain.facts())
```

> **Note:** The Python SDK requires the `amem` binary (Rust core engine). Install it via Step 2 below, or build from source via Step 3.

---

## 2. Rust CLI

The `amem` binary is the core engine. Use it standalone or as the backend for the Python SDK. Requires **Rust 1.70+**.

```bash
cargo install agentic-memory
```

This installs the `amem` command-line tool.

### Verify

```bash
amem --help
amem create test.amem
amem add test.amem fact "Installation successful" --session 1
amem info test.amem
```

### Available commands

**Core commands:**

| Command | Description |
|:---|:---|
| `amem create` | Create a new empty `.amem` file |
| `amem add` | Add a cognitive event (fact, decision, inference, correction, skill, episode) |
| `amem link` | Add an edge between two nodes |
| `amem info` | Display file information |
| `amem traverse` | Run a graph traversal from a starting node |
| `amem search` | Find nodes matching conditions |
| `amem impact` | Run causal impact analysis |
| `amem resolve` | Follow SUPERSEDES chains to current truth |
| `amem export` | Export graph as JSON |
| `amem import` | Import from JSON |
| `amem stats` | Detailed graph statistics |

**v0.2 query commands (9 new):**

| Command | Description |
|:---|:---|
| `amem text-search` | BM25 text search (1.58 ms @ 100K with index) |
| `amem hybrid-search` | Combined BM25 + vector search via RRF |
| `amem centrality` | PageRank, degree, or betweenness centrality |
| `amem path` | Shortest path (BFS or Dijkstra) between two nodes |
| `amem revise` | Counterfactual belief revision analysis |
| `amem gaps` | Detect reasoning weaknesses and gaps |
| `amem analogy` | Find structurally similar past patterns |
| `amem consolidate` | Dedup, contradiction linking, inference promotion |
| `amem drift` | Track belief evolution over time |

All commands support `--json` output for programmatic consumption.

---

## 3. MCP Server (for Claude Desktop, VS Code, Cursor, Windsurf)

The MCP server exposes a brain as 12 tools, 6 resources, and 4 prompts to any MCP-compatible LLM client.

```bash
cargo install agentic-memory-mcp
```

### Configure Claude Desktop

Add to `~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "agentic-memory": {
      "command": "agentic-memory-mcp",
      "args": ["serve"]
    }
  }
}
```

> Zero-config: defaults to `~/.brain.amem`. Override with `"args": ["--memory", "/path/to/brain.amem", "serve"]`.

### Configure VS Code / Cursor

Add to `.vscode/settings.json`:

```json
{
  "mcp.servers": {
    "agentic-memory": {
      "command": "agentic-memory-mcp",
      "args": ["serve"]
    }
  }
}
```

### Verify

Once connected, the LLM gains access to tools like `memory_add`, `memory_query`, `memory_traverse`, `memory_correct`, `memory_resolve`, `memory_similar`, and more. Test by asking the LLM to store a fact:

> "Remember that this project uses PostgreSQL 16."

The LLM should call `memory_add` and confirm the event was stored.

See the [MCP server README](crates/agentic-memory-mcp/README.md) for the full tool/resource/prompt reference.

---

## 4. Auto-Installer (connects all your AI tools)

The auto-installer scans your machine for AI tools and connects them all to a shared AgenticMemory brain file.

```bash
pip install amem-installer
```

### Usage

```bash
# Auto-detect and configure all tools
amem-install install --auto

# Preview what would be configured (dry run)
amem-install install --dry-run

# Check connection status
amem-install status

# Remove all configurations
amem-install uninstall

# Re-scan for new tools
amem-install update
```

### What it does

1. Scans your system for installed AI tools
2. Creates a shared brain file at `~/.brain.amem`
3. Configures each tool to use the shared brain (via MCP servers, config files, or wrapper scripts)
4. Backs up all modified configs before making changes

All modifications are additive — existing configurations are never deleted.

### Supported tools

| Tool | Detection | Integration |
|:---|:---|:---|
| Claude Code | Config file | MCP server |
| Claude Desktop | Config file | MCP server |
| Cursor | Config file | MCP server |
| Windsurf | Config file | MCP server |
| Continue | Config file | Context provider |
| OpenClaw | Config file | YAML config |
| Ollama | HTTP service | Wrapper script |
| LM Studio | HTTP service | Config file |
| LangChain | requirements.txt | Instructions |
| CrewAI | requirements.txt | Instructions |
| AutoGen | requirements.txt | Instructions |

### Example output

```
AgenticMemory Installer
-----------------------

Scanning for AI tools...
  Claude Code          ~/.claude.json
  Claude Desktop       ~/Library/Application Support/Claude/claude_desktop_config.json
  Cursor               ~/.cursor/mcp.json
  Windsurf             ~/.codeium/windsurf/mcp_config.json
  Ollama               Running (3 model(s) available)

Will configure 5 tool(s). Proceed? [Y/n] Y

  [1/5] Claude Code          ... configured (MCP server)
  [2/5] Claude Desktop       ... configured (MCP server)
  [3/5] Cursor               ... configured (MCP server)
  [4/5] Windsurf             ... configured (MCP server)
  [5/5] Ollama               ... configured (wrapper script)

Done! Brain file: ~/.brain.amem
All 5 tools now share persistent memory.
```

---

## 5. Remote Server (coming in v0.2.0)

> **Preview** — these features are under development. Track progress in [#1](https://github.com/agentralabs/agentic-memory/issues/1).

```bash
# Remote single-user
agentic-memory-mcp serve-http \
  --memory /data/brain.amem \
  --port 8080 \
  --token "secret123"

# Remote multi-tenant
agentic-memory-mcp serve-http \
  --multi-tenant \
  --data-dir /data/users/ \
  --port 8080 \
  --token "secret123"
```

Docker compose with Caddy reverse proxy will also be available. See the [v0.2.0 roadmap](https://github.com/agentralabs/agentic-memory/issues/1) for details.

---

## Build from Source

```bash
git clone https://github.com/agentralabs/agentic-memory.git
cd agentic-memory

# Build entire workspace (core library + MCP server)
cargo build --release

# Install core CLI
cargo install --path crates/agentic-memory

# Install MCP server
cargo install --path crates/agentic-memory-mcp

# Install Python SDK (development mode)
cd python
pip install -e ".[dev]"

# Install auto-installer (development mode)
cd ../installer
pip install -e ".[dev]"
```

### Run tests

```bash
# All workspace tests (core + MCP + bridge: 314 tests)
cargo test --workspace

# Core library only (179 tests)
cargo test -p agentic-memory

# MCP server only (119 tests)
cargo test -p agentic-memory-mcp

# Bridge integration tests (16 tests)
cargo test -p agentic-memory-bridge-tests

# Python SDK tests (104 tests)
cd python && pytest tests/ -v

# Installer tests (39 tests)
cd ../installer && pytest tests/ -v
```

---

## Package Registry Links

| Package | Registry | Install |
|:---|:---|:---|
| **agentic-memory** | [crates.io](https://crates.io/crates/agentic-memory) | `cargo install agentic-memory` |
| **agentic-memory-mcp** | [crates.io](https://crates.io/crates/agentic-memory-mcp) | `cargo install agentic-memory-mcp` |
| **agentic-brain** | [PyPI](https://pypi.org/project/agentic-brain/) | `pip install agentic-brain` |
| **amem-installer** | [PyPI](https://pypi.org/project/amem-installer/) | `pip install amem-installer` |

---

## Requirements

| Component | Minimum version |
|:---|:---|
| Python | 3.10+ |
| Rust | 1.70+ (only for building from source or `cargo install`) |
| OS | macOS, Linux, Windows |

---

## Troubleshooting

### `pip: command not found`

Use `pip3` instead, or the full path to your Python:

```bash
python3 -m pip install agentic-brain
```

### `amem: command not found` after `cargo install`

Make sure `~/.cargo/bin` is in your PATH:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

Add this line to your `~/.zshrc` or `~/.bashrc` to make it permanent.

### `amem-install: command not found` after `pip install`

The script may be installed to a user-local bin directory. Try:

```bash
python3 -m amem_installer.cli install --auto
```

Or find where pip installed it:

```bash
python3 -m pip show amem-installer
```

### Python SDK says "amem binary not found"

Install the Rust core engine first:

```bash
cargo install agentic-memory
```

Or build from source if you don't have Rust:

```bash
git clone https://github.com/agentralabs/agentic-memory.git
cd agentic-memory
cargo build --release
cp target/release/amem /usr/local/bin/
```
