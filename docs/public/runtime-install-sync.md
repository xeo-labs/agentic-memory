# Runtime, Install Output, and Sync Contract

This page defines expected runtime behavior across installer output, CLI behavior, and web documentation.

## Installer profiles

- `desktop`: installs binaries and merges detected desktop MCP config.
- `terminal`: installs binaries without desktop-specific UX assumptions.
- `server`: installs binaries without desktop config writes.

## Completion output contract

Installer must print:

1. Installed binary summary.
2. MCP restart instruction.
3. Server auth + artifact sync guidance when relevant.
4. Optional feedback instruction.

Expected completion marker:

```text
Install complete: AgenticMemory (<profile>)
```

## Universal MCP config

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

## Server auth + sync

```bash
export AGENTIC_TOKEN="$(openssl rand -hex 32)"
```

Server deployments must sync `.amem/.acb/.avis` artifacts to server storage before runtime.
