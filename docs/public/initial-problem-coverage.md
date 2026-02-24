# Initial Problem Coverage (Memory)

This page records the **foundational problems AgenticMemory already solved** before the newer primary-problem expansion.

## Reference set

| Ref | Initial problem solved | Shipped capability |
|---|---|---|
| IAM-I01 | No durable cross-session memory | `.amem` artifact + session continuity |
| IAM-I02 | No typed memory model | event types: fact/decision/inference/correction/skill/episode |
| IAM-I03 | No correction lineage | `correction` + `supersedes` + `amem resolve` |
| IAM-I04 | No precise retrieval controls | `amem search`, `amem hybrid-search`, filters/sort |
| IAM-I05 | No memory-health diagnostics | `amem quality`, MCP `memory_quality` |
| IAM-I06 | No runtime handoff sync | `amem runtime-sync --write-episode` |
| IAM-I07 | No long-horizon memory budget control | `amem budget` + storage budget policy env vars |
| IAM-I08 | No universal MCP memory runtime | `agentic-memory-mcp` tools/resources |

## AgenticCodebase verification snapshot

Verification method used: AgenticCodebase scanning AgenticMemory source.

```bash
acb -f json compile <agentic-memory-repo> -o /tmp/acb_memory_repo.acb --exclude target --exclude .git --include-tests
acb -f json info /tmp/acb_memory_repo.acb
acb -f json query /tmp/acb_memory_repo.acb symbol --name quality
acb -f json query /tmp/acb_memory_repo.acb symbol --name runtime_sync
acb -f json query /tmp/acb_memory_repo.acb symbol --name budget
acb -f json query /tmp/acb_memory_repo.acb symbol --name resolve
```

Observed snapshot (2026-02-24):

- Units: `3527`
- Edges: `4427`
- Languages: `3`
- Compile status: `ok`
- Symbol evidence:
  - `commands::cmd_quality`
  - `commands::cmd_runtime_sync`
  - `commands::cmd_budget`
  - `memory_resolve` module + resolve tests

## Status

All initial references `IAM-I01` to `IAM-I08` are implemented and actively testable from CLI/MCP surfaces.

## See also

- [Primary Problem Coverage](primary-problem-coverage.md)
- [Quickstart](quickstart.md)
