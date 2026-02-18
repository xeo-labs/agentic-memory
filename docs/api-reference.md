# Python API Reference

Complete reference for the `agentic_memory` Python package. Install with `pip install agentic-memory`.

## Brain

The primary class for interacting with an AgenticMemory graph. Each `Brain` instance corresponds to a single `.amem` file.

### Constructor

```python
Brain(path: str | Path)
```

Opens an existing brain file or creates a new one at the given path. A new session is started automatically on each instantiation.

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `path` | `str \| Path` | Path to the `.amem` file. Created if it does not exist. |

**Raises:** `BrainError` if the file exists but is corrupted or has an incompatible version.

**Example:**

```python
from agentic_memory import Brain

brain = Brain("my_agent.amem")
brain = Brain(Path("/data/agents/assistant.amem"))
```

---

### add_fact()

```python
Brain.add_fact(
    content: str,
    confidence: float = 1.0,
    metadata: dict[str, str] | None = None
) -> Event
```

Stores a fact event -- externally observed or received information.

**Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `content` | `str` | *required* | The textual content of the fact. |
| `confidence` | `float` | `1.0` | Confidence score between 0.0 and 1.0. |
| `metadata` | `dict[str, str] \| None` | `None` | Optional key-value metadata. |

**Returns:** `Event` -- the newly created event with its assigned ID.

---

### add_decision()

```python
Brain.add_decision(
    content: str,
    confidence: float = 1.0,
    metadata: dict[str, str] | None = None
) -> Event
```

Stores a decision event -- a choice or judgment the agent has made.

**Parameters:** Same as `add_fact()`.

**Returns:** `Event`

---

### add_inference()

```python
Brain.add_inference(
    content: str,
    confidence: float = 1.0,
    metadata: dict[str, str] | None = None
) -> Event
```

Stores an inference event -- a conclusion derived from existing knowledge.

**Parameters:** Same as `add_fact()`.

**Returns:** `Event`

---

### add_correction()

```python
Brain.add_correction(
    content: str,
    confidence: float = 1.0,
    metadata: dict[str, str] | None = None
) -> Event
```

Stores a correction event -- an update that revises previous knowledge. Typically followed by a `supersedes` edge linking the correction to the event it replaces.

**Parameters:** Same as `add_fact()`.

**Returns:** `Event`

---

### add_skill()

```python
Brain.add_skill(
    content: str,
    confidence: float = 1.0,
    metadata: dict[str, str] | None = None
) -> Event
```

Stores a skill event -- a learned capability or reusable procedure.

**Parameters:** Same as `add_fact()`.

**Returns:** `Event`

---

### add_episode()

```python
Brain.add_episode(
    content: str,
    confidence: float = 1.0,
    metadata: dict[str, str] | None = None
) -> Event
```

Stores an episode event -- a narrative summary of an interaction or experience.

**Parameters:** Same as `add_fact()`.

**Returns:** `Event`

---

### link()

```python
Brain.link(
    source: int,
    target: int,
    edge_type: str | EdgeType,
    weight: float = 1.0
) -> Edge
```

Creates a directed, weighted edge between two events.

**Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `source` | `int` | *required* | ID of the source event. |
| `target` | `int` | *required* | ID of the target event. |
| `edge_type` | `str \| EdgeType` | *required* | One of: `"caused_by"`, `"supports"`, `"contradicts"`, `"supersedes"`, `"related_to"`, `"part_of"`, `"temporal_next"`. |
| `weight` | `float` | `1.0` | Edge weight between 0.0 and 1.0. |

**Returns:** `Edge`

**Raises:** `BrainError` if either node ID does not exist, or if the edge type is invalid.

---

### facts()

```python
Brain.facts(session: int | None = None) -> list[Event]
```

Returns all fact events, optionally filtered to a specific session.

**Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `session` | `int \| None` | `None` | If provided, only return facts from this session. |

**Returns:** `list[Event]`

---

### decisions()

```python
Brain.decisions(session: int | None = None) -> list[Event]
```

Returns all decision events, optionally filtered to a specific session.

**Parameters:** Same as `facts()`.

**Returns:** `list[Event]`

---

### traverse()

```python
Brain.traverse(
    start: int,
    depth: int = 3,
    edge_types: list[str | EdgeType] | None = None
) -> TraversalResult
```

Performs a breadth-first traversal of the graph starting from the given node.

**Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `start` | `int` | *required* | ID of the starting node. |
| `depth` | `int` | `3` | Maximum traversal depth. |
| `edge_types` | `list[str \| EdgeType] \| None` | `None` | If provided, only follow edges of these types. |

**Returns:** `TraversalResult`

---

### resolve()

```python
Brain.resolve(event_id: int) -> Event
```

Follows the `supersedes` chain from the given event to find the most current version. If no supersedes edge exists, returns the original event.

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `event_id` | `int` | ID of the event to resolve. |

**Returns:** `Event` -- the most current version in the supersedes chain.

---

### impact()

```python
Brain.impact(event_id: int, depth: int = 5) -> ImpactResult
```

Analyzes the downstream impact of an event by traversing all outgoing edges (reverse direction). Returns all events that depend on, were caused by, or reference the given event.

**Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `event_id` | `int` | *required* | ID of the event to analyze. |
| `depth` | `int` | `5` | Maximum traversal depth. |

**Returns:** `ImpactResult`

---

### info()

```python
Brain.info() -> BrainInfo
```

Returns summary information about the brain.

**Returns:** `BrainInfo`

---

### session_info()

```python
Brain.session_info(session: int) -> SessionInfo
```

Returns detailed information about a specific session.

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `session` | `int` | The session ID to query. |

**Returns:** `SessionInfo`

**Raises:** `BrainError` if the session does not exist.

---

### search()

```python
Brain.search(
    query: str,
    top_k: int = 10,
    event_type: str | EventType | None = None,
    session: int | None = None,
    min_confidence: float = 0.0
) -> list[SearchResult]
```

Performs semantic similarity search across all events using 128-dimensional feature vectors.

**Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `query` | `str` | *required* | Natural language search query. |
| `top_k` | `int` | `10` | Maximum number of results to return. |
| `event_type` | `str \| EventType \| None` | `None` | Filter results to a specific event type. |
| `session` | `int \| None` | `None` | Filter results to a specific session. |
| `min_confidence` | `float` | `0.0` | Minimum confidence threshold. |

**Returns:** `list[SearchResult]` -- results sorted by descending similarity score.

---

## MemoryAgent

Connects a `Brain` to an LLM provider for automatic memory extraction from conversations.

### Constructor

```python
MemoryAgent(
    brain: Brain,
    provider: LLMProvider,
    auto_link: bool = True,
    extraction_prompt: str | None = None
)
```

**Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `brain` | `Brain` | *required* | The brain to store extracted memories in. |
| `provider` | `LLMProvider` | *required* | An LLM provider instance. |
| `auto_link` | `bool` | `True` | Automatically create edges between extracted events and relevant existing events. |
| `extraction_prompt` | `str \| None` | `None` | Custom system prompt for memory extraction. Uses a sensible default if not provided. |

---

### chat()

```python
MemoryAgent.chat(
    message: str,
    context: list[Event] | None = None
) -> str
```

Sends a message to the LLM with relevant memory context, returns the response, and extracts new cognitive events from the conversation.

**Parameters:**

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `message` | `str` | *required* | The user message to process. |
| `context` | `list[Event] \| None` | `None` | Additional events to include in the LLM context. If `None`, relevant events are retrieved automatically via similarity search. |

**Returns:** `str` -- the LLM's response text.

---

### last_extraction

```python
MemoryAgent.last_extraction: list[Event]
```

A read-only property containing the list of events extracted from the most recent `chat()` call. Empty if no extraction occurred.

---

## Data Classes

### Event

Represents a single cognitive event in the graph.

```python
@dataclass
class Event:
    id: int                          # Unique node ID within the brain
    event_type: EventType            # Fact, Decision, Inference, Correction, Skill, Episode
    content: str                     # The textual content
    session: int                     # Session ID this event belongs to
    confidence: float                # Confidence score (0.0 to 1.0)
    timestamp: datetime              # UTC timestamp of creation
    metadata: dict[str, str]         # Optional key-value metadata
```

### Edge

Represents a directed, weighted relationship between two events.

```python
@dataclass
class Edge:
    source: int                      # Source node ID
    target: int                      # Target node ID
    edge_type: EdgeType              # Relationship type
    weight: float                    # Edge weight (0.0 to 1.0)
```

### BrainInfo

Summary information about a brain.

```python
@dataclass
class BrainInfo:
    node_count: int                  # Total number of events
    edge_count: int                  # Total number of edges
    session_count: int               # Number of sessions
    file_size: int                   # File size in bytes
    sessions: list[int]              # List of session IDs
    version: int                     # File format version
```

### SessionInfo

Detailed information about a single session.

```python
@dataclass
class SessionInfo:
    id: int                          # Session ID
    node_count: int                  # Number of events in this session
    edge_count: int                  # Number of edges between session events
    start_time: datetime             # Timestamp of the first event
    end_time: datetime               # Timestamp of the last event
    event_types: dict[str, int]      # Count of each event type
```

### TraversalResult

Result of a graph traversal operation.

```python
@dataclass
class TraversalResult:
    nodes: list[Event]               # All nodes reached during traversal
    edges: list[Edge]                # All edges traversed
    depth_reached: int               # Maximum depth actually reached
```

### ImpactResult

Result of an impact analysis.

```python
@dataclass
class ImpactResult:
    affected: list[Event]            # Events downstream of the analyzed event
    edges: list[Edge]                # Edges in the impact graph
    total_affected: int              # Total count of affected events
```

### SearchResult

A single result from a similarity search.

```python
@dataclass
class SearchResult:
    event: Event                     # The matching event
    score: float                     # Similarity score (0.0 to 1.0)
```

---

## Enums

### EventType

```python
class EventType(str, Enum):
    FACT = "fact"
    DECISION = "decision"
    INFERENCE = "inference"
    CORRECTION = "correction"
    SKILL = "skill"
    EPISODE = "episode"
```

### EdgeType

```python
class EdgeType(str, Enum):
    CAUSED_BY = "caused_by"
    SUPPORTS = "supports"
    CONTRADICTS = "contradicts"
    SUPERSEDES = "supersedes"
    RELATED_TO = "related_to"
    PART_OF = "part_of"
    TEMPORAL_NEXT = "temporal_next"
```

---

## Exceptions

### BrainError

```python
class BrainError(Exception):
    """Raised for brain file operations: corruption, invalid IDs, I/O failures."""
    pass
```

### CLIError

```python
class CLIError(Exception):
    """Raised when the underlying Rust CLI returns an error."""
    pass
```

### ProviderError

```python
class ProviderError(Exception):
    """Raised for LLM provider failures: API errors, rate limits, invalid responses."""
    pass
```

---

## LLMProvider (Abstract Base)

Base class for implementing custom LLM providers.

```python
from abc import ABC, abstractmethod
from agentic_memory import Event

class LLMProvider(ABC):

    @abstractmethod
    def complete(self, prompt: str, system: str | None = None) -> str:
        """Send a prompt to the LLM and return the completion text.

        Args:
            prompt: The user/input prompt.
            system: Optional system prompt.

        Returns:
            The LLM's response text.

        Raises:
            ProviderError: If the API call fails.
        """
        ...

    @abstractmethod
    def extract_events(self, text: str) -> list[dict]:
        """Extract cognitive events from text.

        The LLM should identify facts, decisions, inferences, etc.
        in the input text and return them as structured dictionaries.

        Args:
            text: The text to extract events from.

        Returns:
            List of dicts with keys: "type", "content", "confidence".

        Raises:
            ProviderError: If extraction fails.
        """
        ...

    def embed(self, text: str) -> list[float] | None:
        """Generate an embedding vector for the given text.

        Optional. If not implemented, the default internal embedding
        model is used. Return a list of 128 floats.

        Args:
            text: The text to embed.

        Returns:
            A 128-dimensional float vector, or None to use the default.
        """
        return None
```

### Implementing a Custom Provider

```python
from agentic_memory.providers import LLMProvider, ProviderError

class MyCustomProvider(LLMProvider):

    def __init__(self, api_url: str, api_key: str):
        self.api_url = api_url
        self.api_key = api_key

    def complete(self, prompt: str, system: str | None = None) -> str:
        # Call your LLM API here
        response = requests.post(
            f"{self.api_url}/completions",
            headers={"Authorization": f"Bearer {self.api_key}"},
            json={"prompt": prompt, "system": system}
        )
        if response.status_code != 200:
            raise ProviderError(f"API error: {response.status_code}")
        return response.json()["text"]

    def extract_events(self, text: str) -> list[dict]:
        extraction_prompt = f"Extract cognitive events from: {text}"
        raw = self.complete(extraction_prompt, system="Extract facts, decisions, inferences...")
        # Parse the LLM output into structured events
        return parse_extraction(raw)

# Usage
provider = MyCustomProvider("https://my-llm.example.com", "my-api-key")
agent = MemoryAgent(brain, provider)
```

---

## Built-in Providers

### AnthropicProvider

```python
from agentic_memory.providers import AnthropicProvider

provider = AnthropicProvider(
    api_key: str = None,             # Defaults to ANTHROPIC_API_KEY env var
    model: str = "claude-sonnet-4-20250514",
)
```

### OpenAIProvider

```python
from agentic_memory.providers import OpenAIProvider

provider = OpenAIProvider(
    api_key: str = None,             # Defaults to OPENAI_API_KEY env var
    model: str = "gpt-4o",
)
```

### OllamaProvider

```python
from agentic_memory.providers import OllamaProvider

provider = OllamaProvider(
    model: str = "llama3.1",
    host: str = "http://localhost:11434",
)
```
