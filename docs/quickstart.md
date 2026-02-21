# Quickstart Guide

Get AgenticMemory running in under 5 minutes. This guide covers installation, creating your first brain, storing cognitive events, querying memory, and connecting an LLM provider.

## Installation

### Python SDK

```bash
pip install agentic-brain
```

Requires Python 3.10 or later. The package includes pre-built binaries for macOS (ARM and x86), Linux (x86_64), and Windows (x86_64). No Rust toolchain needed.

### Rust CLI

```bash
cargo install agentic-memory
```

This installs the `amem` binary. Requires Rust 1.70 or later (tested with 1.90.0).

### From Source

```bash
git clone https://github.com/agentralabs/agentic-memory.git
cd agentic-memory
cargo build --release
pip install -e python/
```

## Create Your First Brain

A **Brain** is the top-level container for all of an agent's memory. It maps to a single `.amem` file on disk.

### Python

```python
from agentic_memory import Brain

brain = Brain("my_agent.amem")
print(brain.info())
# BrainInfo(nodes=0, edges=0, sessions=0, file_size=64)
```

### CLI

```bash
amem create my_agent.amem
amem info my_agent.amem
```

## Add Cognitive Events

AgenticMemory stores six types of cognitive events. Start with facts and decisions:

```python
# Store a fact the agent has learned
fact = brain.add_fact(
    "The project deadline is March 15, 2026",
    confidence=0.95,
    metadata={"source": "user_message", "topic": "project"}
)

# Store a decision the agent has made
decision = brain.add_decision(
    "Prioritize backend API work over frontend styling",
    confidence=0.85,
    metadata={"reasoning": "deadline_driven"}
)

# Store an inference derived from other knowledge
inference = brain.add_inference(
    "The team will need to work weekends to meet the March deadline",
    confidence=0.70,
    metadata={"basis": "timeline_analysis"}
)

print(f"Stored fact: {fact.id}")
print(f"Stored decision: {decision.id}")
print(f"Stored inference: {inference.id}")
```

### CLI Equivalents

```bash
amem add my_agent.amem fact "The project deadline is March 15, 2026" --confidence 0.95
amem add my_agent.amem decision "Prioritize backend API work over frontend styling" --confidence 0.85
```

## Link Events Together

Events become powerful when connected. Edges capture relationships between cognitive events:

```python
# The inference was caused by the fact
brain.link(fact.id, inference.id, "caused_by", weight=0.9)

# The decision is supported by the inference
brain.link(decision.id, inference.id, "supports", weight=0.8)
```

### CLI

```bash
amem link my_agent.amem 0 2 caused_by --weight 0.9
amem link my_agent.amem 1 2 supports --weight 0.8
```

## Query Memory

Retrieve events by type, search by content similarity, or traverse the graph:

```python
# Get all facts
all_facts = brain.facts()
for f in all_facts:
    print(f"{f.content} (confidence: {f.confidence})")

# Get all decisions
all_decisions = brain.decisions()

# Traverse the graph from a starting node
result = brain.traverse(fact.id, depth=3)
for node in result.nodes:
    print(f"  [{node.event_type}] {node.content}")

# Hybrid search (BM25 + vector) across all events
matches = brain.search("project timeline", top_k=5)
for match in matches:
    print(f"  Score: {match.score:.3f} | {match.event.content}")
```

### CLI

```bash
amem query my_agent.amem --event-types fact
amem traverse my_agent.amem 0 --depth 3
amem query my_agent.amem --search "project timeline" --top-k 5
```

## v0.2 Advanced Queries

v0.2.0 added nine new query types for retrieval, structural analysis, cognitive reasoning, and graph maintenance:

```python
# BM25 text search (1.58 ms @ 100K nodes)
results = brain.search_text("project timeline")

# Hybrid BM25 + vector search (10.83 ms @ 100K nodes)
results = brain.search("project timeline", top_k=10)

# Structural: PageRank centrality (34.3 ms @ 100K)
scores = brain.centrality(metric="pagerank")

# Structural: Shortest path via BFS (104 µs @ 100K)
path = brain.shortest_path(src=fact.id, dst=decision.id)

# Cognitive: Belief revision cascade (53.4 ms @ 100K)
report = brain.revise(node_id=fact.id)

# Cognitive: Find reasoning gaps
gaps = brain.gaps()

# Maintenance: Detect belief drift (68.4 ms @ 100K)
drift = brain.drift()
```

### CLI

```bash
amem text-search my_agent.amem "project timeline"
amem hybrid-search my_agent.amem "project timeline" --top-k 10
amem centrality my_agent.amem --metric pagerank
amem path my_agent.amem 0 2
amem revise my_agent.amem 0
amem gaps my_agent.amem
amem drift my_agent.amem
```

## Add Corrections

When knowledge changes, use corrections with supersedes edges to maintain an audit trail:

```python
# The deadline changed
correction = brain.add_correction(
    "The project deadline has been moved to April 1, 2026",
    confidence=0.98,
    metadata={"source": "manager_update"}
)

# Mark the old fact as superseded
brain.link(correction.id, fact.id, "supersedes")

# Resolve the latest truth for a chain
current = brain.resolve(fact.id)
print(current.content)
# "The project deadline has been moved to April 1, 2026"
```

## Connect an LLM Provider

The `MemoryAgent` class connects a Brain to an LLM, enabling automatic memory extraction from conversations:

```python
from agentic_memory import Brain, MemoryAgent
from agentic_memory.integrations import AnthropicProvider

brain = Brain("assistant.amem")
provider = AnthropicProvider(api_key="sk-ant-...")

agent = MemoryAgent(brain, provider)

# Chat with automatic memory extraction
response = agent.chat("My name is Alice and I work at Acme Corp as a senior engineer.")
print(response)
# The agent responds AND automatically extracts facts:
#   - "The user's name is Alice"
#   - "Alice works at Acme Corp"
#   - "Alice's role is senior engineer"

# See what was extracted
for event in agent.last_extraction:
    print(f"  [{event.event_type}] {event.content}")
```

### With OpenAI

```python
from agentic_memory.integrations import OpenAIProvider

provider = OpenAIProvider(api_key="sk-...")
agent = MemoryAgent(brain, provider)
```

### With Local Ollama

```python
from agentic_memory.integrations import OllamaProvider

provider = OllamaProvider(model="llama3.1")  # No API key needed
agent = MemoryAgent(brain, provider)
```

## Inspect Your Brain

```python
info = brain.info()
print(f"Nodes: {info.node_count}")
print(f"Edges: {info.edge_count}")
print(f"Sessions: {info.session_count}")
print(f"File size: {info.file_size} bytes")

# Session-level detail
for session in info.sessions:
    s = brain.session_info(session)
    print(f"  Session {s.id}: {s.node_count} events, {s.edge_count} edges")
```

## MCP Server (for Claude Desktop, VS Code, Cursor)

The MCP server gives any MCP-compatible LLM client access to the full AgenticMemory engine — 12 tools, 6 resources, and 4 prompts over JSON-RPC 2.0.

### Install

```bash
cargo install agentic-memory-mcp
```

### Configure Claude Desktop

Add to `~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "agentic-memory": {
      "command": "agentic-memory-mcp",
      "args": ["serve"]
    }
  }
}
```

> Zero-config: defaults to `~/.brain.amem`. Override with `"args": ["--memory", "/path/to/brain.amem", "serve"]`.

### Test It

Restart Claude Desktop and ask:

> "Remember that this project uses Rust with the Tokio async runtime."

Claude will call `memory_add` to store the fact. Later, ask:

> "What do you remember about this project's tech stack?"

Claude will call `memory_query` or `memory_similar` to retrieve the stored knowledge.

See the [MCP server README](../crates/agentic-memory-mcp/README.md) for the full tool/resource/prompt reference.

---

## Next Steps

- **[Core Concepts](concepts.md)** -- Understand the cognitive event model, edge semantics, and memory formation pipeline.
- **[Python API Reference](api-reference.md)** -- Complete reference for all classes and methods.
- **[Rust API Reference](rust-api.md)** -- Rust library and CLI documentation.
- **[Integration Guide](integration-guide.md)** -- Connect AgenticMemory to Claude, GPT, LangChain, CrewAI, and MCP servers.
- **[File Format Specification](file-format.md)** -- Deep dive into the `.amem` binary format.
- **[Benchmarks](benchmarks.md)** -- Performance characteristics at various scales.
- **[FAQ](faq.md)** -- Common questions and answers.
