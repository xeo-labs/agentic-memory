# AgenticMemory Examples

Runnable examples demonstrating the AgenticMemory Python SDK.

## Prerequisites

```bash
pip install agentic-memory
amem-install --auto --yes
```

For examples that use LLM providers, set the relevant API keys:

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
export OPENAI_API_KEY="sk-..."
```

## Examples

| File | Description |
|------|-------------|
| `01_basic_usage.py` | Simplest possible example. Create a brain, add facts and a decision, link them with causal edges, and query. |
| `02_build_knowledge.py` | Multi-session knowledge building. Add facts, make decisions, then correct outdated information. Shows `resolve()` and `impact()`. |
| `03_reasoning_chains.py` | Build a decision tree with CAUSED_BY and SUPPORTS edges, then use `traverse()` to reconstruct the full reasoning chain. |
| `04_cross_provider.py` | Use the same brain with different LLM providers (Anthropic, OpenAI). Knowledge persists across provider switches. |
| `05_agent_chat.py` | Full conversational agent with persistent memory across multiple chat sessions. |
| `06_one_command_install.sh` | Bash script showing the recommended install flow. |

## Running

```bash
# No API key needed
python examples/01_basic_usage.py
python examples/02_build_knowledge.py
python examples/03_reasoning_chains.py

# Requires at least one API key
python examples/04_cross_provider.py
python examples/05_agent_chat.py

# Install flow
bash examples/06_one_command_install.sh
```
