# Integration Guide

This guide covers how to integrate AgenticMemory into various environments: direct Python usage, LLM providers, agent frameworks, AI coding assistants via MCP, and custom provider implementations.

## Direct Python Usage

The simplest integration uses the `Brain` class directly without any LLM. You control what gets stored and how events are linked.

```python
from agentic_memory import Brain, EventType

brain = Brain("project.amem")

# Store events manually
fact = brain.add_fact("The API rate limit is 1000 requests per minute", confidence=0.99)
decision = brain.add_decision("Implement client-side rate limiting with exponential backoff")
brain.link(decision.id, fact.id, "caused_by", weight=0.9)

# Query later
facts = brain.facts()
results = brain.search("rate limiting", top_k=5)
related = brain.traverse(fact.id, depth=3)
```

This approach works well when:
- You have structured data and know exactly what to store.
- You want full control over the memory formation process.
- You are building a pipeline where events are extracted by external logic.

---

## With Anthropic Claude

The `AnthropicProvider` connects AgenticMemory to Claude models for automatic memory extraction.

### Setup

```bash
pip install agentic-memory anthropic
```

### Basic Usage

```python
from agentic_memory import Brain, MemoryAgent
from agentic_memory.providers import AnthropicProvider

brain = Brain("claude_assistant.amem")
provider = AnthropicProvider(
    api_key="sk-ant-...",            # Or set ANTHROPIC_API_KEY env var
    model="claude-sonnet-4-20250514",      # Default model
)

agent = MemoryAgent(brain, provider)

# Chat with automatic memory extraction
response = agent.chat("I'm working on a Django project that needs to handle 50K concurrent users.")
print(response)

# See what was extracted
for event in agent.last_extraction:
    print(f"  [{event.event_type.value}] {event.content} (confidence: {event.confidence})")
```

### Using Environment Variables

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
```

```python
provider = AnthropicProvider()  # Picks up ANTHROPIC_API_KEY automatically
```

### Multi-turn Conversations

The `MemoryAgent` accumulates context across calls within a session. Each `chat()` call retrieves relevant past memories and adds them to the LLM context.

```python
agent.chat("My name is Alice and I'm a backend engineer at Acme Corp.")
agent.chat("We use PostgreSQL and Redis in production.")
agent.chat("What database setup would you recommend for our new microservice?")
# Claude's response will reference Alice's name, role, and existing tech stack
```

---

## With OpenAI GPT

The `OpenAIProvider` works with GPT-4o, GPT-4, and other OpenAI models.

### Setup

```bash
pip install agentic-memory openai
```

### Usage

```python
from agentic_memory import Brain, MemoryAgent
from agentic_memory.providers import OpenAIProvider

brain = Brain("gpt_assistant.amem")
provider = OpenAIProvider(
    api_key="sk-...",               # Or set OPENAI_API_KEY env var
    model="gpt-4o",                 # Default model
)

agent = MemoryAgent(brain, provider)
response = agent.chat("Let's plan the architecture for a real-time chat application.")
```

### Custom Model Selection

```python
# Use GPT-4 Turbo for more complex extraction
provider = OpenAIProvider(model="gpt-4-turbo")

# Use GPT-4o Mini for faster, cheaper operation
provider = OpenAIProvider(model="gpt-4o-mini")
```

---

## With Local Ollama Models

The `OllamaProvider` connects to locally-running Ollama models. No API keys required, all data stays on your machine.

### Setup

```bash
# Install Ollama (macOS)
brew install ollama

# Pull a model
ollama pull llama3.1

# Start the server (if not already running)
ollama serve
```

```bash
pip install agentic-memory
```

### Usage

```python
from agentic_memory import Brain, MemoryAgent
from agentic_memory.providers import OllamaProvider

brain = Brain("local_assistant.amem")
provider = OllamaProvider(
    model="llama3.1",
    host="http://localhost:11434",   # Default Ollama endpoint
)

agent = MemoryAgent(brain, provider)
response = agent.chat("Track my project milestones: MVP by March, beta by May, launch by July.")
```

### Recommended Models

| Model | Size | Extraction Quality | Speed |
|-------|------|-------------------|-------|
| `llama3.1:70b` | 40 GB | Excellent | Slow |
| `llama3.1` (8B) | 4.7 GB | Good | Fast |
| `mistral` (7B) | 4.1 GB | Good | Fast |
| `phi3:medium` | 7.9 GB | Good | Moderate |

Larger models produce better extraction results but require more VRAM and run slower.

---

## LangChain Integration

AgenticMemory can be used as a memory backend for LangChain chains and agents via a thin wrapper.

### Setup

```bash
pip install agentic-memory langchain langchain-anthropic
```

### As a LangChain Memory Backend

```python
from agentic_memory import Brain
from agentic_memory.integrations.langchain import AgenticMemoryWrapper
from langchain_anthropic import ChatAnthropic
from langchain.chains import ConversationChain

brain = Brain("langchain_agent.amem")
memory = AgenticMemoryWrapper(brain)

llm = ChatAnthropic(model="claude-sonnet-4-20250514")
chain = ConversationChain(llm=llm, memory=memory)

response = chain.invoke({"input": "My favorite programming language is Rust."})
# The fact is automatically stored in the brain

response = chain.invoke({"input": "What's my favorite language?"})
# Memory is retrieved and included in context
```

### Custom Retrieval in LangChain

```python
from langchain.schema import BaseRetriever, Document

class AgenticMemoryRetriever(BaseRetriever):
    def __init__(self, brain: Brain, top_k: int = 5):
        self.brain = brain
        self.top_k = top_k

    def _get_relevant_documents(self, query: str) -> list[Document]:
        results = self.brain.search(query, top_k=self.top_k)
        return [
            Document(
                page_content=r.event.content,
                metadata={
                    "event_type": r.event.event_type.value,
                    "confidence": r.event.confidence,
                    "score": r.score,
                }
            )
            for r in results
        ]
```

---

## CrewAI Integration

AgenticMemory can serve as persistent memory for CrewAI agents.

### Setup

```bash
pip install agentic-memory crewai
```

### Usage

```python
from agentic_memory import Brain
from agentic_memory.integrations.crewai import AgenticMemoryTool
from crewai import Agent, Task, Crew

brain = Brain("crew_memory.amem")
memory_tool = AgenticMemoryTool(brain)

researcher = Agent(
    role="Research Analyst",
    goal="Research and remember key findings",
    tools=[memory_tool],
    verbose=True
)

task = Task(
    description="Research the latest trends in AI agent architectures and store your findings.",
    agent=researcher
)

crew = Crew(agents=[researcher], tasks=[task])
result = crew.kickoff()

# All findings are now in the brain file
print(f"Stored {brain.info().node_count} events")
```

---

## Claude Code (MCP Server)

AgenticMemory includes an MCP (Model Context Protocol) server that exposes a brain as tools for Claude Code.

### Start the MCP Server

```bash
amem mcp-serve project.amem --port 3100
```

### Configure Claude Code

Add to your Claude Code MCP configuration (typically `~/.claude/mcp.json`):

```json
{
  "mcpServers": {
    "memory": {
      "command": "amem",
      "args": ["mcp-serve", "/path/to/project.amem", "--port", "3100"]
    }
  }
}
```

### Available MCP Tools

Once connected, Claude Code gains access to these tools:

| Tool | Description |
|------|-------------|
| `memory_add_fact` | Store a new fact. |
| `memory_add_decision` | Store a decision. |
| `memory_add_inference` | Store an inference. |
| `memory_search` | Search memories by semantic similarity. |
| `memory_recall` | Retrieve events by type, session, or ID. |
| `memory_link` | Create edges between events. |
| `memory_info` | Get brain statistics. |

Claude Code will automatically use these tools during conversations to remember context, recall prior decisions, and maintain continuity across sessions.

---

## Cursor / Windsurf (MCP Config)

### Cursor

Add to your Cursor MCP settings (Settings > MCP):

```json
{
  "mcpServers": {
    "agentic-memory": {
      "command": "amem",
      "args": ["mcp-serve", "/path/to/project.amem"],
      "env": {}
    }
  }
}
```

Alternatively, create a `.cursor/mcp.json` file in your project root:

```json
{
  "mcpServers": {
    "agentic-memory": {
      "command": "amem",
      "args": ["mcp-serve", "./project.amem"]
    }
  }
}
```

### Windsurf

Add to your Windsurf MCP configuration (`~/.windsurf/mcp.json` or project-level `.windsurf/mcp.json`):

```json
{
  "mcpServers": {
    "agentic-memory": {
      "command": "amem",
      "args": ["mcp-serve", "/path/to/project.amem"]
    }
  }
}
```

### Verifying the Connection

After configuring, restart your editor. You should see the memory tools available in the tool list. Test by asking the assistant to store a fact:

> "Remember that this project uses PostgreSQL 16 with pgvector for embeddings."

The assistant should call `memory_add_fact` and confirm the event was stored.

---

## Building a Custom LLMProvider

If your LLM is not covered by the built-in providers, implement the `LLMProvider` interface.

### Required Methods

```python
from agentic_memory.providers import LLMProvider, ProviderError

class MyProvider(LLMProvider):

    def __init__(self, endpoint: str, api_key: str):
        self.endpoint = endpoint
        self.api_key = api_key
        self.session = requests.Session()
        self.session.headers["Authorization"] = f"Bearer {api_key}"

    def complete(self, prompt: str, system: str | None = None) -> str:
        """Generate a completion from the LLM.

        This is called for general chat responses. The MemoryAgent
        uses this to generate replies to user messages.
        """
        payload = {"prompt": prompt}
        if system:
            payload["system"] = system

        resp = self.session.post(f"{self.endpoint}/v1/completions", json=payload)
        if resp.status_code != 200:
            raise ProviderError(f"API returned {resp.status_code}: {resp.text}")

        return resp.json()["choices"][0]["text"]

    def extract_events(self, text: str) -> list[dict]:
        """Extract cognitive events from conversation text.

        This is called after each chat() call to identify facts,
        decisions, inferences, and other events in the conversation.

        Must return a list of dicts with keys:
          - "type": one of "fact", "decision", "inference", "correction", "skill", "episode"
          - "content": the textual content of the event
          - "confidence": float between 0.0 and 1.0
        """
        extraction_prompt = (
            "Analyze the following conversation and extract cognitive events.\n"
            "For each event, identify its type (fact/decision/inference/correction/skill/episode),\n"
            "content, and confidence score (0.0-1.0).\n"
            "Return JSON array.\n\n"
            f"Conversation:\n{text}"
        )

        raw = self.complete(extraction_prompt, system="You are a memory extraction system.")
        try:
            events = json.loads(raw)
            return [
                {"type": e["type"], "content": e["content"], "confidence": e.get("confidence", 0.8)}
                for e in events
            ]
        except (json.JSONDecodeError, KeyError) as err:
            raise ProviderError(f"Failed to parse extraction: {err}")
```

### Optional: Custom Embeddings

Override the `embed()` method if your LLM provides embeddings:

```python
    def embed(self, text: str) -> list[float] | None:
        """Generate a 128-dimensional embedding vector.

        Return None to use the default internal embedding model.
        If you return a vector, it MUST be exactly 128 floats.
        """
        resp = self.session.post(
            f"{self.endpoint}/v1/embeddings",
            json={"input": text, "dimensions": 128}
        )
        if resp.status_code != 200:
            return None  # Fall back to default

        vector = resp.json()["data"][0]["embedding"]
        return vector[:128]  # Ensure correct dimension
```

### Using Your Custom Provider

```python
from agentic_memory import Brain, MemoryAgent

brain = Brain("custom.amem")
provider = MyProvider("https://my-llm.example.com", "my-key")
agent = MemoryAgent(brain, provider)

response = agent.chat("Store some information for me.")
```

### Testing Your Provider

AgenticMemory includes a provider test suite you can run against your implementation:

```python
from agentic_memory.testing import run_provider_tests

provider = MyProvider("https://my-llm.example.com", "my-key")
results = run_provider_tests(provider)
print(results.summary())
```

This tests:
- Basic completion works.
- Event extraction returns valid typed events.
- Embedding vectors (if provided) have the correct dimension.
- Error handling for API failures.
