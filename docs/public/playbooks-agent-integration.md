---
status: stable
---

# AI Agent Integration

Get your AI agent working with AgenticMemory in 30 seconds.

---

## 30-Second Start

**MCP (Claude Desktop, Cursor, Windsurf):**

```bash
curl -fsSL https://raw.githubusercontent.com/agentralabs/agentic-memory/main/scripts/install.sh | bash
```

Restart your client. Done. Your agent now has persistent memory.

**Python:**

```bash
pip install agentic-memory
```

```python
from agentic_memory import MemoryGraph
graph = MemoryGraph("agent.amem")
graph.add_fact("User prefers Python")
graph.save()
```

---

## System Prompt Templates

Add one of these to your agent's system prompt.

### Minimal

```
You have persistent memory via AgenticMemory.
- Use memory_store to save important information
- Use memory_query to recall relevant context
```

### Standard (Recommended)

```
You have persistent memory via AgenticMemory MCP server.

STORING MEMORIES:
- memory_store type="fact" → User preferences, stated information
- memory_store type="decision" → Choices made with reasoning
- memory_store type="insight" → Patterns you've noticed

RETRIEVING MEMORIES:
- memory_query → Search by keywords or type
- memory_recent → Get latest memories
- memory_context → Get memories relevant to current task

RULES:
1. Check memory at conversation start for relevant context
2. Store new facts when user shares preferences or information
3. Store decisions when choices are made
4. Reference memories naturally, don't announce "checking memory"
```

### Full (Production Agents)

```
You have persistent memory via AgenticMemory MCP server.

MEMORY TYPES:
- FACT: Verified information ("User is a Python developer")
- DECISION: Choices with reasoning ("Chose PostgreSQL for ACID compliance")
- INSIGHT: Patterns observed ("User prefers concise responses")
- OBSERVATION: Contextual notes ("Working on e-commerce project")
- CORRECTION: Updated information (supersedes previous facts)
- QUESTION: Unresolved queries to follow up on

WHEN TO STORE:
- User states a preference → FACT
- User shares personal/professional info → FACT
- A decision is made → DECISION with reasoning
- You notice a pattern → INSIGHT
- Information changes → CORRECTION (links to original)

WHEN TO RETRIEVE:
- Start of conversation → memory_context for relevant background
- Before recommendations → memory_query for preferences
- When user says "remember" or "like before" → memory_recent

MEMORY HYGIENE:
- Confidence: 0.9+ for explicit statements, 0.7 for inferred, 0.5 for uncertain
- Don't store: transient info, sensitive data, obvious facts
- Do store: anything you'd want to know in 6 months

BEHAVIOR:
- Retrieve silently, use naturally
- Never say "let me check my memory" or "according to my records"
- Speak as if you simply know the person
```

---

## Example Prompts

What users say and what memory operations to perform:

| User Says | Memory Action |
|-----------|---------------|
| "I prefer dark mode" | `memory_store type=fact content="User prefers dark mode"` |
| "Let's use React for this" | `memory_store type=decision content="Using React" reasoning="User choice"` |
| "What do you know about me?" | `memory_query type=fact limit=20` |
| "Remember last time we..." | `memory_recent limit=10` then `memory_query` on topic |
| "I changed my mind, use Vue" | `memory_store type=correction content="Using Vue" supersedes=[id]` |

---

## Common Patterns

### Preference Learning

Store preferences as revealed, retrieve before recommendations.

```python
# Store
graph.add_fact("User prefers functional style", confidence=0.9, tags=["coding"])

# Retrieve
prefs = graph.query_by_type("fact", limit=10)
relevant = [p for p in prefs if "coding" in p.tags]
```

### Decision Tracking

Track decisions with reasoning for future reference.

```python
graph.add_decision(
    content="Selected PostgreSQL",
    reasoning="ACID compliance, team experience",
    tags=["architecture", "database"]
)
```

### Context Injection

Load relevant context at conversation start.

```python
def start_conversation(topic: str) -> str:
    context = graph.query_similar(topic, limit=5)
    recent = graph.query_recent(limit=3)
    return "\n".join([f"- {m.content}" for m in context + recent])
```

### Correction Chain

Handle updated information properly.

```python
original_id = graph.add_fact("Favorite language is Python")

# Later, user changes mind
graph.add_correction(
    content="Favorite language is now Rust",
    supersedes=original_id
)
```

---

## Framework Integration

### LangChain

```python
from agentic_memory import MemoryGraph

class AgenticMemoryWrapper:
    def __init__(self, path: str):
        self.graph = MemoryGraph(path)

    def load_memory_variables(self, inputs: dict) -> dict:
        query = inputs.get("input", "")
        memories = self.graph.query_similar(query, limit=5)
        return {"history": "\n".join([f"- {m.content}" for m in memories])}

    def save_context(self, inputs: dict, outputs: dict) -> None:
        self.graph.add_observation(content=f"User: {inputs['input']}")
        self.graph.save()
```

### CrewAI

```python
from agentic_memory import MemoryGraph

crew_memory = MemoryGraph("crew_shared.amem")

def get_context(topic: str) -> str:
    memories = crew_memory.query_similar(topic, limit=5)
    return "\n".join([m.content for m in memories])

def store_finding(content: str, agent: str) -> None:
    crew_memory.add_insight(content=content, metadata={"source": agent})
    crew_memory.save()
```

### Raw Python

```python
from agentic_memory import MemoryGraph
from openai import OpenAI

client = OpenAI()
graph = MemoryGraph("assistant.amem")

def chat(user_message: str) -> str:
    # Get context
    context = graph.query_similar(user_message, limit=5)
    context_str = "\n".join([f"- {m.content}" for m in context])

    # Call LLM
    response = client.chat.completions.create(
        model="gpt-4",
        messages=[
            {"role": "system", "content": f"Context:\n{context_str}"},
            {"role": "user", "content": user_message}
        ]
    )

    return response.choices[0].message.content
```

---

## MCP Tool Reference

| Tool | Purpose | Key Parameters |
|------|---------|----------------|
| `memory_store` | Store new memory | `type`, `content`, `confidence`, `tags` |
| `memory_query` | Search memories | `keywords`, `type`, `limit` |
| `memory_recent` | Get latest | `limit` |
| `memory_context` | Get relevant context | `topic`, `limit` |
| `memory_correct` | Update memory | `original_id`, `new_content` |

---

## Troubleshooting

**Memory not persisting?**
- Ensure `graph.save()` is called (Python)
- Check file permissions on `.amem` file
- MCP: Restart client after install

**Context not relevant?**
- Use more specific tags
- Increase query limit
- Enable embeddings for semantic search

---

## Next Steps

- [Memory Core Concepts](/docs/en/memory-concepts)
- [Memory API Reference](/docs/en/memory-api-reference)
- [Memory Benchmarks](/docs/en/memory-benchmarks)
