# Command Surface

Install commands are documented in [Installation](installation.md).

## Binaries

- `amem` (CLI engine)
- `agentic-memory-mcp` (MCP server)

## `amem` top-level

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
amem quality
amem runtime-sync
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

## `agentic-memory-mcp` commands

```bash
agentic-memory-mcp serve
agentic-memory-mcp validate
agentic-memory-mcp info
agentic-memory-mcp delete
agentic-memory-mcp export
agentic-memory-mcp compact
agentic-memory-mcp stats
```

## New reliability commands

```bash
amem quality my_agent.amem
amem quality my_agent.amem --low-confidence 0.4 --stale-decay 0.2 --limit 25

amem runtime-sync my_agent.amem --workspace . --max-depth 4
amem runtime-sync my_agent.amem --workspace . --write-episode
```

## MCP quality tool

- `memory_quality` returns a graph reliability summary (confidence, staleness, orphan nodes, unsupported decisions).

## Universal MCP entry

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

## Artifact

- Primary artifact: `.amem`
