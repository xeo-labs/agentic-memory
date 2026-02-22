# Command Surface (Canonical Sync Source)

This page is an authoritative command catalog for AgenticMemory and is intended as a source file for web-doc synchronization.

## Install Commands

```bash
# Recommended one-liner
curl -fsSL https://agentralabs.tech/install/memory | bash

# Explicit profiles
curl -fsSL https://agentralabs.tech/install/memory/desktop | bash
curl -fsSL https://agentralabs.tech/install/memory/terminal | bash
curl -fsSL https://agentralabs.tech/install/memory/server | bash

# Rust binaries
cargo install agentic-memory
cargo install agentic-memory-mcp

# Python SDK / installer
pip install agentic-brain
pip install amem-installer
```

## Binaries

- `amem` (CLI engine)
- `agentic-memory-mcp` (MCP server)

## `amem` Top-Level Commands

```bash
amem create
amem info
amem add
amem link
amem get
amem traverse
amem search
amem impact
amem resolve
amem sessions
amem export
amem import
amem decay
amem stats
amem text-search
amem hybrid-search
amem centrality
amem path
amem revise
amem gaps
amem analogy
amem consolidate
amem drift
amem completions
```

## `agentic-memory-mcp` Commands

```bash
agentic-memory-mcp serve
agentic-memory-mcp validate
agentic-memory-mcp info
agentic-memory-mcp delete
agentic-memory-mcp export
agentic-memory-mcp compact
agentic-memory-mcp stats
```

`serve` options include:

- `--memory <file.amem>`
- `--config <file>`
- `--log-level trace|debug|info|warn|error`
- `--mode minimal|smart|full`

## Universal MCP Entry (Any MCP Client)

```json
{
  "mcpServers": {
    "agentic-memory": {
      "command": "$HOME/.local/bin/agentic-memory-mcp",
      "args": ["serve"]
    }
  }
}
```

## Default Memory Artifact

- Default shared brain path: `~/.brain.amem`

## Verification Commands

```bash
# CLI checks
amem --version
amem --help
agentic-memory-mcp --version

# Memory checks
amem create demo.amem
amem add demo.amem fact "memory smoke test"
amem search demo.amem --event-type fact
amem stats demo.amem

# MCP startup check (Ctrl+C after startup)
$HOME/.local/bin/agentic-memory-mcp serve
```

## Artifact Contract

- Primary artifact: `.amem`
- For cross-sister server workflows, sync all required artifacts to server storage: `.amem`, `.acb`, `.avis`

## Publish Commands

```bash
# In repo root
cargo test --workspace
cargo fmt --check
cargo clippy --workspace -- -D warnings

# Dry run (paired crates)
cd crates/agentic-memory && cargo publish --dry-run
cd ../../crates/agentic-memory-mcp && cargo publish --dry-run

# Release (core first)
cd crates/agentic-memory && cargo publish
cd ../../crates/agentic-memory-mcp && cargo publish
```

## Operator Notes

- Desktop/terminal profiles merge MCP config for detected clients.
- Server profile does not write desktop MCP config files.
- After install, restart MCP clients so new config is loaded.
- Optional feedback: https://github.com/agentralabs/agentic-memory/issues
