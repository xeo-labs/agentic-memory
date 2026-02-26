---
status: stable
---

# AgenticMemory Overview

AgenticMemory stores an agent's knowledge as a navigable graph in a single portable `.amem` file.

## What you can do

- Store facts, decisions, inferences, corrections, skills, and episodes as connected graph nodes.
- Query by traversal, similarity, temporal range, causal chains, and quality diagnostics.
- Expose the graph through MCP with `agentic-memory-mcp`.

## Why teams adopt AgenticMemory

Teams adopt AgenticMemory because it closes both the original and current memory gaps:

- Foundational problems already solved: sessions start from zero, vector search returns similar text but no reasoning trails, corrections overwrite truth, memory degrades silently, and long-term memory is not portable.
- New high-scale problems now solved: multi-session context continuity, decision lineage across conversations, memory quality and drift diagnostics at runtime, cross-project knowledge comparison via workspaces, and auto-session lifecycle management.
- Practical outcome for teams: agents remember decisions, correct themselves over time, and carry portable evidence across models, clients, and deployments.

For a detailed before-and-after view, see [Experience With vs Without](experience-with-vs-without.md).

## Artifact

- Primary artifact: `.amem`
- Cross-sister server workflows can pair `.amem` with `.acb` and `.avis`

## Start here

- [Installation](installation.md)
- [Quickstart](quickstart.md)
- [Command Surface](command-surface.md)
- [Runtime and Sync](runtime-install-sync.md)
- [Integration Guide](integration-guide.md)
- [Experience With vs Without](experience-with-vs-without.md)
- [V3 Architecture](v3-architecture.md)

## Works with

- **AgenticCodebase** — link code-graph nodes to memory decisions for traceable reasoning across refactors.
- **AgenticVision** — link visual captures to memory nodes with `vision_link` for cross-modal evidence trails.
