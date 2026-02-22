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
