# Runtime, Install Output, and Sync Contract (Canonical Sync Source)

This page documents runtime behavior that should remain consistent across installer output, CLI behavior, and web documentation.

## Installer Behavior by Profile

- `desktop`
  - Installs `amem` and `agentic-memory-mcp`
  - Merges MCP configs for detected clients
- `terminal`
  - Installs `amem` and `agentic-memory-mcp`
  - Also merges MCP configs
  - Native terminal workflow remains available
- `server`
  - Installs `amem` and `agentic-memory-mcp`
  - Skips desktop config writes
  - Intended for remote/server hosts

## Post-Install Output Contract

The installer emits a completion section with:

1. Installed MCP server command
2. Restart instruction for MCP clients (desktop/terminal profiles)
3. Server auth + artifact sync instruction (server profile)
4. Optional feedback link

Expected completion markers include:

```text
Install complete: AgenticMemory (<profile>)
Done! Memory defaults to ~/.brain.amem
```

## Universal MCP Detection Goal

Any MCP client can consume the same MCP entry:

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

If auto-merge does not detect a client, add the entry manually and restart that client.

## Server Auth Pattern

```bash
TOKEN=$(openssl rand -hex 32)
export AGENTIC_TOKEN="$TOKEN"
# Clients send: Authorization: Bearer $TOKEN
```

## Artifact Sync Rule (Server)

Server runtimes cannot read laptop-local files directly.

Before using Memory + sister data on a server, sync artifacts to server storage:

- `.amem`
- `.acb`
- `.avis`

## Smoke Test Matrix

```bash
# Install simulation
bash scripts/install.sh --dry-run
bash scripts/install.sh --profile=desktop --dry-run
bash scripts/install.sh --profile=terminal --dry-run
bash scripts/install.sh --profile=server --dry-run

# Guardrails
bash scripts/check-install-commands.sh
bash scripts/check-canonical-sister.sh
```

## Release Preflight

```bash
cargo test --workspace
cargo fmt --check
cargo clippy --workspace -- -D warnings
cd crates/agentic-memory && cargo publish --dry-run
cd ../../crates/agentic-memory-mcp && cargo publish --dry-run
```

## Support

- Install and runtime issues: https://github.com/agentralabs/agentic-memory/issues
