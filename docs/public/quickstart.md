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
