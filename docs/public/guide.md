# AgenticMemory Guide

AgenticMemory provides long-horizon cognitive memory through a portable `.amem` artifact and an MCP server.

## What it does

- Stores typed memory events across sessions (`fact`, `decision`, `inference`, `correction`, `skill`, `episode`).
- Supports precise retrieval with search and hybrid search.
- Tracks correction lineage with supersedes/resolve flows.
- Exposes memory operations to MCP clients through `agentic-memory-mcp`.

## Why teams adopt AgenticMemory

Teams adopt AgenticMemory because it closes both foundational and modern memory-runtime problems:

- Foundational problems already solved: no durable cross-session memory, no typed memory model, no correction lineage, weak retrieval controls, no memory quality diagnostics, poor runtime handoff continuity, no long-horizon storage policy, and no universal MCP memory surface.
- New high-scale problems now solved: context-window loss, retrieval noise, cross-session amnesia, contradiction persistence, weak uncertainty handling, long-term storage governance, privacy/redaction controls, feedback incorporation lag, and poor handoff quality.
- Practical outcome for teams: agents retain relevant context over time, correct errors explicitly, and transfer sessions with less drift and less repeated user effort.

For full reference mapping, see:

- [Initial Problem Coverage](initial-problem-coverage.md)
- [Primary Problem Coverage](primary-problem-coverage.md)

## Artifact

- Primary artifact: `.amem`
- Cross-sister server workflows can pair `.amem` with `.acb` and `.avis`

## Start here

- [Installation](installation.md)
- [Quickstart](quickstart.md)
- [Command Surface](command-surface.md)
- [Runtime and Sync](runtime-install-sync.md)
- [Integration Guide](integration-guide.md)
