# Primary Problem Coverage (Memory)

This page tracks direct coverage for Memory primary problems:

- P01 context-window limits
- P02 retrieval noise
- P05 cross-session amnesia
- P06 contradiction persistence
- P07 weak uncertainty calibration
- P32 long-horizon storage governance
- P33 privacy/redaction control
- P34 feedback incorporation lag
- P42 handoff quality gaps

## What is implemented now

Memory already provides the core long-term graph runtime. This phase adds an explicit regression entrypoint:

```bash
./scripts/test-primary-problems.sh
```

The script validates:

1. Longitudinal event capture with confidence
2. Retrieval controls (`search`, `hybrid-search`)
3. Correction and resolve flows (`add correction`, `resolve`, `revise`)
4. Quality status and uncertainty surfaces (`quality`)
5. Cross-session continuity (`runtime-sync`, `sessions`)
6. 20-year storage governance (`budget`)
7. Focused Rust + MCP regression tests

## Problem-to-capability map

| Problem | Coverage primitive |
|---|---|
| P01 | `.amem` persistence + `search`/`runtime-sync` |
| P02 | `search` filters + `hybrid-search` weights |
| P05 | `sessions`, `runtime-sync --write-episode` |
| P06 | `revise`, correction edges, `resolve` |
| P07 | confidence fields + `quality` thresholds |
| P32 | `budget` + budget env policy |
| P33 | redaction controls (`AMEM_AUTO_CAPTURE_REDACT=true`) |
| P34 | correction events + supersedes chain |
| P42 | episode snapshots via runtime sync |

## Notes on P33 privacy

Runtime redaction policy is operationally controlled via env vars:

```bash
export AMEM_AUTO_CAPTURE_MODE=safe
export AMEM_AUTO_CAPTURE_REDACT=true
export AMEM_AUTO_CAPTURE_MAX_CHARS=2048
```

Use `safe` mode by default for public and shared environments.

## See also

- [Initial Problem Coverage](initial-problem-coverage.md)
