# Quickstart

## 1. Install

```bash
curl -fsSL https://agentralabs.tech/install/memory | bash
```

Profile-specific commands are listed in [Installation](installation.md).

## 2. Create a brain

```bash
amem create my_agent.amem
amem info my_agent.amem
```

## 3. Add and query memory

```bash
amem add my_agent.amem fact "The project deadline is March 15, 2026" --confidence 0.95
amem search my_agent.amem --event-type fact
amem quality my_agent.amem
amem runtime-sync my_agent.amem --workspace . --write-episode
amem budget my_agent.amem --horizon-years 20 --max-bytes 2147483648
```

## 4. Start MCP server

```bash
$HOME/.local/bin/agentic-memory-mcp serve
```

Use `Ctrl+C` to stop after startup verification.

## 5. Validate MCP quality output

Run:

```bash
agentic-memory-mcp info
```

Expected tools include `memory_quality`.

## 6. Enable automatic long-horizon budget enforcement

```bash
export AMEM_STORAGE_BUDGET_MODE=auto-rollup
export AMEM_STORAGE_BUDGET_BYTES=2147483648
export AMEM_STORAGE_BUDGET_HORIZON_YEARS=20
```

Optional:

```bash
export AMEM_STORAGE_BUDGET_TARGET_FRACTION=0.85
```

When enabled, maintenance ticks auto-roll up completed sessions into episode summaries when budget pressure is detected.

## 7. Enable prompt and feedback auto-capture

```bash
export AMEM_AUTO_CAPTURE_MODE=safe
export AMEM_AUTO_CAPTURE_REDACT=true
export AMEM_AUTO_CAPTURE_MAX_CHARS=2048
```

Modes:

- `safe`: capture prompt templates plus explicit feedback/session summary fields (`feedback`, `summary`, `note`).
- `full`: capture broader tool input text (except direct `memory_add` payload duplication).
- `off`: disable auto-capture.
