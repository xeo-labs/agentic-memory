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

## Long-horizon storage budget policy

To target ~1-2 GB over long horizons (for example 20 years), configure:

```bash
export AMEM_STORAGE_BUDGET_MODE=auto-rollup
export AMEM_STORAGE_BUDGET_BYTES=2147483648
export AMEM_STORAGE_BUDGET_HORIZON_YEARS=20
export AMEM_STORAGE_BUDGET_TARGET_FRACTION=0.85
```

Modes:

- `auto-rollup`: auto-archive completed sessions into episode summaries when budget pressure is detected.
- `warn`: emit warnings only.
- `off`: disable policy.

## Prompt and feedback capture policy

For automatic prompt/context persistence into `.amem`, configure:

```bash
export AMEM_AUTO_CAPTURE_MODE=safe
export AMEM_AUTO_CAPTURE_REDACT=true
export AMEM_AUTO_CAPTURE_MAX_CHARS=2048
```

Modes:

- `safe`: capture prompt template input and feedback-style fields with minimal noise.
- `full`: capture broader tool input context (except direct `memory_add` duplication).
- `off`: disable auto-capture.
