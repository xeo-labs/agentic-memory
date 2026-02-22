# Implementation Report (2026-02-22)

This page records the memory-system upgrades implemented in this cycle.

## What was added

1. Graph quality engine:
   - New `QueryEngine::memory_quality(...)` API.
   - Evaluates low-confidence nodes, stale nodes, orphan nodes, unsupported decisions, contradiction/supersedes edges.
   - Returns `pass|warn|fail` status plus example node IDs.
2. CLI quality command:
   - `amem quality <file> [--low-confidence] [--stale-decay] [--limit]`
3. Runtime sync command:
   - `amem runtime-sync <file> --workspace <path> [--max-depth] [--write-episode]`
   - Scans `.amem`, `.acb`, `.avis` artifacts and can write an Episode snapshot for session continuity.
4. MCP quality tool:
   - New `memory_quality` tool in `agentic-memory-mcp`.
   - Exposes the same reliability report to any MCP client.
5. Long-horizon storage budget policy:
   - Added `amem budget` command for projection and budget guidance.
   - Added runtime policy in MCP session manager:
     - `AMEM_STORAGE_BUDGET_MODE=auto-rollup|warn|off`
     - `AMEM_STORAGE_BUDGET_BYTES`
     - `AMEM_STORAGE_BUDGET_HORIZON_YEARS`
     - `AMEM_STORAGE_BUDGET_TARGET_FRACTION`
   - `auto-rollup` mode compresses completed sessions into episode summaries when budget pressure is detected.
6. Prompt + feedback auto-capture:
   - MCP `tools/call` and `prompts/get` now support automatic capture into `.amem`.
   - Runtime policy:
     - `AMEM_AUTO_CAPTURE_MODE=safe|full|off`
     - `AMEM_AUTO_CAPTURE_REDACT=true|false`
     - `AMEM_AUTO_CAPTURE_MAX_CHARS`
   - `safe` mode focuses on prompt templates and explicit feedback/session-summary fields.
   - `full` mode captures broader tool input context while skipping direct `memory_add` payload duplication.

## Why this matters

- Improves memory trustworthiness and operational visibility.
- Adds a concrete session-resync mechanism for local artifact presence.
- Keeps CLI and MCP surfaces consistent, so desktop/terminal/server clients get equivalent diagnostics.

## Verified commands

```bash
amem quality /tmp/agentra-demo.amem
amem runtime-sync /tmp/agentra-demo.amem --workspace /Users/omoshola/Documents --max-depth 2
amem budget /tmp/agentra-demo.amem --horizon-years 20 --max-bytes 2147483648
agentic-memory-mcp info
```

## Files changed

- `crates/agentic-memory/src/engine/query.rs`
- `crates/agentic-memory/src/engine/mod.rs`
- `crates/agentic-memory/src/lib.rs`
- `crates/agentic-memory/src/cli/commands.rs`
- `crates/agentic-memory/src/bin/amem.rs`
- `crates/agentic-memory/tests/phase5_quality.rs`
- `crates/agentic-memory-mcp/src/tools/memory_quality.rs`
- `crates/agentic-memory-mcp/src/tools/mod.rs`
- `crates/agentic-memory-mcp/src/tools/registry.rs`
- `crates/agentic-memory-mcp/src/session/manager.rs`
