# AgenticMemory Python SDK

Python SDK for AgenticMemory — portable binary graph memory for AI agents.

## Install

```bash
pip install agentic-memory
```

### With LLM integrations

```bash
pip install agentic-memory[anthropic]   # Claude
pip install agentic-memory[openai]      # GPT
pip install agentic-memory[ollama]      # Local models
pip install agentic-memory[all]         # All providers
```

## Quick Start

```python
from agentic_memory import Brain

brain = Brain("my_agent.amem")
brain.add_fact("User is a Python developer", session=1)
brain.add_decision("Recommended FastAPI for REST APIs", session=1)

print(brain.facts())
print(brain.info())
```

## With LLM Integration

```python
from agentic_memory import Brain, MemoryAgent
from agentic_memory.integrations import AnthropicProvider

brain = Brain("my_agent.amem")
agent = MemoryAgent(brain, AnthropicProvider())

response = agent.chat("My name is Alice. I work on ML systems.", session=1)
response = agent.chat("What do I work on?", session=2)
```

## Requirements

- Python >= 3.10
- `amem` binary (Rust core engine) — install via `cargo install amem`

## Documentation

- [API Reference](../docs/api-reference.md)
- [Integration Guide](../docs/integration-guide.md)
- [Full README](../README.md)

## License

MIT
