# Quickstart Guide

Get AgenticMemory running in under 5 minutes. This guide covers installation, creating your first brain, storing cognitive events, querying memory, and connecting an LLM provider.

## Installation

### Python SDK

```bash
pip install agentic-memory
```

Requires Python 3.9 or later. The package includes pre-built binaries for macOS (ARM and x86), Linux (x86_64), and Windows (x86_64). No Rust toolchain needed.

### Rust CLI

```bash
cargo install agentic-memory
```

This installs the `amem` binary. Requires Rust 1.70 or later.

### From Source

```bash
git clone https://github.com/anthropic/agentic-memory.git
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

# Semantic search across all events
matches = brain.search("project timeline", top_k=5)
for match in matches:
    print(f"  Score: {match.score:.3f} | {match.event.content}")
```

### CLI

```bash
amem query my_agent.amem --type fact
amem traverse my_agent.amem 0 --depth 3
amem query my_agent.amem --search "project timeline" --top-k 5
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
from agentic_memory.providers import AnthropicProvider

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
from agentic_memory.providers import OpenAIProvider

provider = OpenAIProvider(api_key="sk-...")
agent = MemoryAgent(brain, provider)
```

### With Local Ollama

```python
from agentic_memory.providers import OllamaProvider

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

## Next Steps

- **[Core Concepts](concepts.md)** -- Understand the cognitive event model, edge semantics, and memory formation pipeline.
- **[Python API Reference](api-reference.md)** -- Complete reference for all classes and methods.
- **[Rust API Reference](rust-api.md)** -- Rust library and CLI documentation.
- **[Integration Guide](integration-guide.md)** -- Connect AgenticMemory to Claude, GPT, LangChain, CrewAI, and MCP servers.
- **[File Format Specification](file-format.md)** -- Deep dive into the `.amem` binary format.
- **[Benchmarks](benchmarks.md)** -- Performance characteristics at various scales.
- **[FAQ](faq.md)** -- Common questions and answers.
