# Core Concepts

AgenticMemory models an AI agent's knowledge as a **cognitive graph** -- a directed, weighted graph where nodes represent discrete cognitive events and edges capture the relationships between them. This document explains the foundational ideas behind the system.

## Why a Graph?

Most agent memory systems store flat lists of text chunks or key-value pairs. This works for simple retrieval but fails to capture how knowledge actually relates: facts support decisions, decisions cause outcomes, new information supersedes old information, and errors get corrected.

A graph structure preserves these relationships explicitly. When an agent recalls a decision, it can also traverse to the facts that supported it, the inferences that led to it, and any corrections that followed. This makes agent reasoning auditable, debuggable, and improvable.

Compared to vector databases, which answer "what is similar?", a cognitive graph answers richer questions: "what caused this?", "what does this contradict?", "what superseded this?", and "what was the chain of reasoning?"

## Cognitive Events (Nodes)

Every node in the graph is a **cognitive event** -- something the agent observed, concluded, decided, corrected, learned, or experienced. There are six event types, each serving a distinct purpose.

### Fact

A piece of external information the agent has received or observed. Facts originate from user input, API responses, document parsing, or sensor data. They represent the agent's ground truth about the world.

```python
brain.add_fact(
    "Python 3.12 was released on October 2, 2023",
    confidence=0.99,
    metadata={"source": "python.org"}
)
```

**When to use:** The agent receives information from an external source that it treats as true (with some confidence level). Facts are the raw inputs to the agent's reasoning process.

### Decision

A choice the agent has made. Decisions record what the agent chose to do and implicitly capture the context in which the choice was made. Linking decisions to their supporting facts and inferences creates an auditable decision trail.

```python
brain.add_decision(
    "Use PostgreSQL instead of SQLite for the production database",
    confidence=0.88,
    metadata={"alternatives_considered": ["sqlite", "mysql", "postgresql"]}
)
```

**When to use:** The agent commits to a course of action, selects between alternatives, or makes a judgment call. Decisions are the outputs of the agent's reasoning process.

### Inference

A conclusion the agent has drawn from existing knowledge. Inferences are derived from facts, other inferences, or combinations of both. They represent the agent's reasoning -- the bridge between raw observations and decisions.

```python
brain.add_inference(
    "The database will need connection pooling given the expected 10K concurrent users",
    confidence=0.75,
    metadata={"reasoning_method": "capacity_analysis"}
)
```

**When to use:** The agent combines existing knowledge to reach a new conclusion. Inferences should typically be linked (via `caused_by` or `supports`) to the events they were derived from.

### Correction

An update that revises or invalidates previous knowledge. Corrections are how the agent handles changing information, mistakes, or refined understanding. They should be linked to the event they correct via a `supersedes` edge.

```python
correction = brain.add_correction(
    "The release date was actually October 2, 2023, not October 3",
    confidence=0.99,
    metadata={"corrects": "date_error"}
)
brain.link(correction.id, original_fact.id, "supersedes")
```

**When to use:** Previously stored information turns out to be wrong, outdated, or imprecise. Always create a `supersedes` edge from the correction to the event it replaces. The original event is preserved for auditability but `resolve()` will follow the supersedes chain to return the latest version.

### Skill

A learned capability or procedure. Skills record how to do something -- reusable knowledge that the agent can apply across contexts. Unlike facts (which describe the world) or decisions (which are contextual choices), skills represent transferable know-how.

```python
brain.add_skill(
    "To parse CSV files with mixed encodings: detect encoding with chardet, "
    "then use pandas read_csv with the detected encoding parameter",
    confidence=0.90,
    metadata={"language": "python", "domain": "data_processing"}
)
```

**When to use:** The agent learns a reusable procedure, technique, or pattern. Skills are particularly valuable across sessions -- they let the agent carry forward operational knowledge.

### Episode

A narrative record of a complete interaction or experience. Episodes capture the full arc of an event sequence -- what happened, in what order, and what the outcome was. They provide context that individual facts and decisions cannot.

```python
brain.add_episode(
    "Debugged a memory leak in the image processing pipeline. "
    "Root cause was unclosed file handles in the resize function. "
    "Fixed by adding context managers. Took 3 hours.",
    confidence=0.95,
    metadata={"duration_hours": 3, "outcome": "resolved"}
)
```

**When to use:** An interaction or task completes and the agent should record a summary of the experience. Episodes are useful for learning from past experiences and providing context for future similar situations.

## Edges (Relationships)

Edges are directed and weighted. The direction indicates the semantic relationship (A `caused_by` B means "A was caused by B"), and the weight (0.0 to 1.0) indicates the strength of the relationship.

### caused_by

Indicates that one event was caused by or directly resulted from another.

```
"Server crashed" --caused_by--> "Memory leak in image processor"
```

**When to use:** There is a direct causal relationship. Event A happened because of Event B. Use for tracing root causes and understanding chains of consequence.

### supports

Indicates that one event provides evidence for or justification of another.

```
"Use PostgreSQL" --supports--> "PostgreSQL handles 10K concurrent connections"
```

**When to use:** Event B provides evidence, reasoning, or justification that led to or strengthens Event A. Common between decisions and the facts/inferences that back them.

### contradicts

Indicates that two events are in tension or conflict.

```
"System is production-ready" --contradicts--> "3 critical bugs remain unfixed"
```

**When to use:** Two pieces of knowledge cannot both be fully true, or one undermines the other. Contradictions are signals that the agent needs to resolve conflicting information.

### supersedes

Indicates that one event replaces or updates another. This is the primary mechanism for knowledge evolution.

```
"Deadline is April 1" --supersedes--> "Deadline is March 15"
```

**When to use:** New information replaces old information. The `resolve()` method follows supersedes chains to find the most current version. The original event is preserved -- supersedes does not delete, it deprecates.

### related_to

A general association between events that does not fit the more specific edge types.

```
"Team prefers TypeScript" --related_to--> "Frontend rewrite planned"
```

**When to use:** Two events are meaningfully connected but the relationship is not causal, supportive, contradictory, or sequential. Use sparingly -- prefer more specific edge types when they apply.

### part_of

Indicates that one event is a component, phase, or sub-element of another.

```
"Design database schema" --part_of--> "Backend API implementation"
```

**When to use:** Event A is a constituent part of Event B. Common for breaking episodes or complex decisions into their components.

### temporal_next

Indicates chronological sequence -- one event followed another in time.

```
"Deployed to staging" --temporal_next--> "Ran integration tests"
```

**When to use:** Capturing the order of events matters. Use to build timelines within episodes or to record workflow sequences.

## Sessions

A **session** groups events that were created during a single interaction or conversation. Every event belongs to exactly one session. Sessions are created automatically -- each time you open a brain, a new session begins.

Sessions serve multiple purposes:

- **Context isolation:** Events from one conversation are grouped together, making it easy to review what happened during a specific interaction.
- **Temporal organization:** Sessions provide a natural chronological grouping on top of individual event timestamps.
- **Scope control:** Queries can be filtered to a specific session when you only care about events from a particular interaction.

```python
info = brain.session_info(session_id=3)
print(f"Session 3: {info.node_count} events, started at {info.start_time}")
```

## Corrections and Supersedes Chains

Knowledge changes. Deadlines move, requirements shift, mistakes are discovered. AgenticMemory handles this through **correction events** linked by **supersedes edges**, forming a chain of revisions.

```
Correction C  --supersedes-->  Correction B  --supersedes-->  Original Fact A
```

Calling `brain.resolve(A.id)` traverses the supersedes chain and returns Event C -- the most current version. The entire chain is preserved, so you can always audit how knowledge evolved.

This design avoids the problems of in-place mutation:
- The agent never silently loses information.
- You can always trace why a piece of knowledge changed.
- Multiple agents can independently correct the same fact, and the graph records both correction paths.

## Confidence and Decay

Every event carries a **confidence** score between 0.0 and 1.0, representing how certain the agent is about the information. Confidence is set at creation time and can inform downstream reasoning:

- **0.9 -- 1.0:** Near-certain. Directly observed or from highly trusted sources.
- **0.7 -- 0.9:** Confident. Well-supported inferences or reliable sources.
- **0.5 -- 0.7:** Moderate. Plausible but not fully verified.
- **Below 0.5:** Uncertain. Speculative or from unreliable sources.

Confidence is stored as part of the node record and is available for filtering and ranking during queries. Agents can implement their own confidence decay policies on top of the raw values -- for example, reducing confidence on facts that have not been reconfirmed after a certain period.

## Episodes and Narrative Memory

While facts, decisions, and inferences capture discrete pieces of knowledge, **episodes** capture the narrative of an experience. An episode is a higher-level summary that ties together what happened during a task or interaction.

Episodes are particularly valuable for:

- **Learning from experience:** An agent can search past episodes to find situations similar to the current one.
- **Context setting:** Before starting a new task, an agent can review relevant episodes to refresh its understanding.
- **Debugging:** When something goes wrong, episodes provide a human-readable summary of what the agent was doing and why.

A well-structured episode links to the individual events it summarizes via `part_of` edges, creating a two-level view: the high-level narrative and the detailed event graph.

## Memory Formation Pipeline

When using the `MemoryAgent` class with an LLM provider, memory formation follows a four-stage pipeline:

### 1. Observe

The agent receives input -- a user message, an API response, a document, or any other external data. This is raw, unprocessed information.

### 2. Extract

The LLM analyzes the input and identifies discrete cognitive events: facts stated, decisions made, inferences drawn. The provider prompt instructs the LLM to categorize each piece of extracted knowledge by type and assign a confidence score.

### 3. Store

Extracted events are written to the brain as typed nodes. Each node gets a unique ID, a session assignment, a timestamp, and its 128-dimensional feature vector (computed from the content for similarity search).

### 4. Relate

The LLM (or application logic) identifies relationships between the new events and existing knowledge. Edges are created to connect new events to relevant prior events -- linking new facts to existing facts they relate to, new decisions to the inferences that support them, and corrections to the events they supersede.

This pipeline runs automatically in `MemoryAgent.chat()`. When using the `Brain` class directly, you control each stage manually.

## Why Binary Over JSON?

The `.amem` file format is a custom binary format rather than JSON, SQLite, or another text-based format. The reasons:

- **Performance:** Binary records are fixed-size and cache-friendly. Reading a node is a single offset calculation and memory read -- no parsing required. At 100K nodes, this matters enormously.
- **Memory-mapped I/O:** The format is designed for `mmap()` access. The OS manages paging, so you can work with brain files larger than available RAM. This is not practical with JSON.
- **Compact size:** LZ4 compression on content blocks and contiguous vector storage minimize file size. A brain with 100K nodes and 128-dim vectors is roughly 50--60 MB in `.amem` versus 300+ MB in JSON.
- **Portability:** The format is self-contained. A single `.amem` file can be copied between machines, shared between agents, or archived. No database server, no directory structure, no dependencies.
- **Atomic writes:** The format supports atomic write operations. A crash during a write will not corrupt the existing data.

See the [File Format Specification](file-format.md) for the complete binary layout.
