# Changelog

All notable changes to AgenticMemory will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.3] - 2026-02-23

### Fixed
- Publish parity fix: aligned `agentic-memory` and `agentic-memory-mcp` crate versions so crates.io verification uses the matching core API surface.

## [0.2.2] - 2026-02-22

### Fixed
- Hardened stdio framing in MCP server paths for broader desktop/client compatibility.
- Follow-up formatting and release hygiene updates in workspace crates.

### Changed
- Updated workspace documentation for orchestration behavior and install profiles.

## [Unreleased] — v0.2.0 Remote Server Support

### Planned

- **Remote HTTP/SSE transport** (`serve-http` command)
  - `--token` flag for bearer authentication
  - `--multi-tenant --data-dir` for per-user brain files
  - `/health` endpoint for monitoring
  - `--tls-cert` / `--tls-key` for native HTTPS (optional)

- **New CLI commands**
  - `delete` — remove a specific memory node
  - `export` — export brain to JSON
  - `compact` — defragment and optimize brain file

- **Infrastructure**
  - Docker image (`agenticrevolution/agentic-memory-mcp`)
  - docker-compose with Caddy reverse proxy
  - Systemd service file
  - `docs/remote-deployment.md`

- **New error codes**
  - `UNAUTHORIZED (-32803)`, `USER_NOT_FOUND (-32804)`, `RATE_LIMITED (-32805)`

Tracking: [#1](https://github.com/agentralabs/agentic-memory/issues/1)

## [0.2.0] - 2026-02-19

### Added

- **MCP Server (`agentic-memory-mcp` v0.1.0)**
  - 12 MCP tools: `memory_add`, `memory_query`, `memory_traverse`, `memory_correct`, `memory_resolve`, `memory_context`, `memory_similar`, `memory_causal`, `memory_temporal`, `memory_stats`, `session_start`, `session_end`
  - 6 resources via `amem://` URIs (node, session, type index, stats, recent, important)
  - 4 prompt templates: remember, reflect, correct, summarize
  - Stdio transport (default) + optional SSE transport (`--features sse`)
  - Session management with auto-save
  - Confidence validation at MCP layer (0.0-1.0 range enforcement)
  - Published to crates.io: `cargo install agentic-memory-mcp`

- **Monorepo restructure**
  - Cargo workspace with `crates/agentic-memory/` (core) and `crates/agentic-memory-mcp/` (MCP server)
  - Integration bridge tests in `tests/bridge/` (basic, multiagent, concurrent, stress)

- **Query Expansion: 9 new query types (queries 8-16)**
  - BM25 text search with inverted index (fast path) and full-scan fallback (slow path)
  - Hybrid search combining BM25 + vector similarity via Reciprocal Rank Fusion (RRF)
  - Graph centrality: PageRank, degree centrality, and betweenness centrality (Brandes' algorithm)
  - Shortest path: bidirectional BFS (unweighted) and Dijkstra's algorithm (weighted)
  - Belief revision: counterfactual analysis with cascade propagation (read-only)
  - Reasoning gap detection: unjustified decisions, single-source inferences, low-confidence foundations, unstable knowledge, stale evidence
  - Analogical query: structural fingerprinting to find similar past reasoning patterns
  - Consolidation: deduplication, contradiction linking, inference promotion (with dry-run mode)
  - Drift detection: belief trajectory tracking with stability scoring

- **New index structures**
  - TermIndex (tag 0x05): BM25 inverted index with posting lists
  - DocLengths (tag 0x06): dense array of token counts per node
  - Feature flags bitfield in header for forward/backward compatibility

- **9 new CLI commands**
  - `amem text-search`, `amem hybrid-search`, `amem centrality`, `amem path`
  - `amem revise`, `amem gaps`, `amem analogy`, `amem consolidate`, `amem drift`

- **Python SDK additions**
  - 9 new Brain methods: search_text(), search(), centrality(), shortest_path(), revise(), gaps(), analogy(), consolidate(), drift()
  - New result dataclasses in agentic_memory/results.py

- **Backward compatibility**
  - v0.1 files readable by v0.2 code (new queries use slow path)
  - v0.2 files skip unknown index tags gracefully for older readers
  - Feature flags in previously reserved header field

### Test coverage

- Rust core: 179 tests (83 new for v0.2 query methods)
- MCP server: 119 tests (types, protocol, tools, resources, prompts, sessions, streaming, integration, edge cases)
- Bridge integration: 16 tests (basic, multiagent, concurrent, stress)
- Python SDK: 104 tests (20 new for v0.2 query wrappers)
- Total across all suites: 575 tests

### No new dependencies

All algorithms (PageRank, BFS, Dijkstra, BM25) implemented with `std::collections` only.

## [0.1.0] - 2025-02-18

### Added

- **Rust Core Engine**
  - Binary graph format (.amem) with 6 cognitive event types and 7 edge types
  - LZ4-compressed content blocks
  - Memory-mapped I/O for zero-copy access
  - 128-dimensional feature vectors with cosine similarity search
  - Multi-level indexes (type, session, time, cluster)
  - CLI tool (`amem`) with create, add, link, info, traverse, query, mcp-serve commands
  - MCP (Model Context Protocol) server for IDE integration
  - 96 tests passing

- **Python SDK** (`pip install agentic-brain`)
  - `Brain` class wrapping the Rust CLI with full API
  - `MemoryAgent` for LLM-powered agents with persistent memory
  - Provider integrations: Anthropic Claude, OpenAI GPT, Ollama
  - Automatic knowledge extraction from conversations
  - Context injection for memory-aware responses
  - 84 tests passing

- **Terminal Test Agent**
  - Interactive agent with 6 validation protocols
  - Basic recall, decision recall, correction persistence, long-range memory, cross-topic inference, stress testing
  - 97 tests passing

- **Cross-Provider Validation**
  - 21 tests validating memory portability across Claude, GPT, and Ollama
  - Binary format identity verification across providers

- **One-Command Installer**
  - Auto-detection of 11 AI tools (Claude Code, Cursor, Windsurf, Continue, Ollama, LM Studio, etc.)
  - Automatic MCP configuration for supported tools
  - Backup and restore for all modified configs
  - 39 tests passing

- **Research Paper**
  - 7-page publication-grade paper with 7 figures and 6 tables
  - Full benchmark data and methodology

- **Documentation**
  - Quickstart guide, core concepts, API reference, file format specification
  - Integration guides for all supported tools and frameworks
  - FAQ and benchmark documentation
