# Changelog

All notable changes to AgenticMemory will be documented in this file.

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

- **Python SDK** (`pip install agentic-memory`)
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
