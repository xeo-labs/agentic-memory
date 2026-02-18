# Rust API Reference

AgenticMemory is implemented in Rust. This document covers the core library API and the `amem` CLI tool.

## Library Crate

Add to your `Cargo.toml`:

```toml
[dependencies]
agentic-memory = "0.1"
```

---

## CognitiveGraph

The in-memory representation of an agent's cognitive graph. This is the primary data structure for building and manipulating the graph before writing it to disk.

### Constructor

```rust
impl CognitiveGraph {
    /// Creates a new, empty cognitive graph.
    pub fn new() -> Self;
}
```

**Example:**

```rust
use agentic_memory::CognitiveGraph;

let mut graph = CognitiveGraph::new();
```

---

### add_node()

```rust
pub fn add_node(
    &mut self,
    event_type: EventType,
    content: &str,
    session: u32,
    confidence: f32,
    metadata: Option<HashMap<String, String>>,
) -> NodeId;
```

Adds a cognitive event to the graph. Returns the unique `NodeId` for the new node.

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `event_type` | `EventType` | One of: `Fact`, `Decision`, `Inference`, `Correction`, `Skill`, `Episode`. |
| `content` | `&str` | The textual content of the event. |
| `session` | `u32` | Session ID this event belongs to. |
| `confidence` | `f32` | Confidence score between 0.0 and 1.0. |
| `metadata` | `Option<HashMap<String, String>>` | Optional key-value metadata. |

**Example:**

```rust
use agentic_memory::{CognitiveGraph, EventType};

let mut graph = CognitiveGraph::new();
let fact_id = graph.add_node(
    EventType::Fact,
    "Rust 1.75 introduces async fn in traits",
    1,      // session
    0.95,   // confidence
    None,
);
```

---

### add_edge()

```rust
pub fn add_edge(
    &mut self,
    source: NodeId,
    target: NodeId,
    edge_type: EdgeType,
    weight: f32,
) -> Result<EdgeId, GraphError>;
```

Creates a directed, weighted edge between two nodes. Returns the `EdgeId` or an error if either node does not exist.

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `source` | `NodeId` | Source node ID. |
| `target` | `NodeId` | Target node ID. |
| `edge_type` | `EdgeType` | One of: `CausedBy`, `Supports`, `Contradicts`, `Supersedes`, `RelatedTo`, `PartOf`, `TemporalNext`. |
| `weight` | `f32` | Edge weight between 0.0 and 1.0. |

**Example:**

```rust
use agentic_memory::EdgeType;

let decision_id = graph.add_node(EventType::Decision, "Adopt async traits", 1, 0.9, None);
graph.add_edge(decision_id, fact_id, EdgeType::Supports, 0.85)?;
```

---

### node()

```rust
pub fn node(&self, id: NodeId) -> Option<&CognitiveNode>;
```

Returns a reference to the node with the given ID, or `None` if it does not exist.

---

### neighbors()

```rust
pub fn neighbors(
    &self,
    id: NodeId,
    direction: Direction,
) -> Vec<(NodeId, &CognitiveEdge)>;
```

Returns all adjacent nodes and their connecting edges in the specified direction.

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | `NodeId` | The node to query neighbors of. |
| `direction` | `Direction` | `Direction::Outgoing`, `Direction::Incoming`, or `Direction::Both`. |

---

### traverse()

```rust
pub fn traverse(
    &self,
    start: NodeId,
    max_depth: usize,
    edge_filter: Option<&[EdgeType]>,
) -> TraversalResult;
```

Performs a breadth-first traversal from the starting node up to the specified depth.

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `start` | `NodeId` | Starting node for traversal. |
| `max_depth` | `usize` | Maximum depth to traverse. |
| `edge_filter` | `Option<&[EdgeType]>` | If provided, only follow edges of these types. |

**Returns:** `TraversalResult` containing visited nodes, traversed edges, and the depth reached.

---

### node_count() / edge_count()

```rust
pub fn node_count(&self) -> usize;
pub fn edge_count(&self) -> usize;
```

Return the total number of nodes and edges in the graph.

---

### nodes_by_type()

```rust
pub fn nodes_by_type(&self, event_type: EventType) -> Vec<NodeId>;
```

Returns all node IDs matching the given event type.

---

### nodes_by_session()

```rust
pub fn nodes_by_session(&self, session: u32) -> Vec<NodeId>;
```

Returns all node IDs belonging to the given session.

---

## Types

### EventType

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventType {
    Fact,
    Decision,
    Inference,
    Correction,
    Skill,
    Episode,
}
```

### EdgeType

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeType {
    CausedBy,
    Supports,
    Contradicts,
    Supersedes,
    RelatedTo,
    PartOf,
    TemporalNext,
}
```

### CognitiveNode

```rust
pub struct CognitiveNode {
    pub id: NodeId,
    pub event_type: EventType,
    pub content: String,
    pub session: u32,
    pub confidence: f32,
    pub timestamp: i64,                          // Unix timestamp (seconds)
    pub metadata: HashMap<String, String>,
    pub vector: Option<Vec<f32>>,                // 128-dim feature vector
}
```

### CognitiveEdge

```rust
pub struct CognitiveEdge {
    pub source: NodeId,
    pub target: NodeId,
    pub edge_type: EdgeType,
    pub weight: f32,
}
```

### TraversalResult

```rust
pub struct TraversalResult {
    pub nodes: Vec<NodeId>,
    pub edges: Vec<(NodeId, NodeId, EdgeType)>,
    pub depth_reached: usize,
}
```

### Direction

```rust
pub enum Direction {
    Outgoing,
    Incoming,
    Both,
}
```

### GraphError

```rust
#[derive(Debug, thiserror::Error)]
pub enum GraphError {
    #[error("node {0} not found")]
    NodeNotFound(NodeId),
    #[error("invalid edge: {0}")]
    InvalidEdge(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("format error: {0}")]
    Format(String),
}
```

---

## File I/O

### FileWriter

Serializes a `CognitiveGraph` to the `.amem` binary format.

```rust
use agentic_memory::io::FileWriter;

impl FileWriter {
    /// Writes the graph to the specified file path.
    /// Creates the file if it does not exist; overwrites if it does.
    pub fn write(graph: &CognitiveGraph, path: &Path) -> Result<(), GraphError>;
}
```

**Example:**

```rust
use agentic_memory::io::FileWriter;
use std::path::Path;

FileWriter::write(&graph, Path::new("agent.amem"))?;
```

### FileReader

Deserializes a `.amem` file into a `CognitiveGraph`. Reads the entire file into memory.

```rust
use agentic_memory::io::FileReader;

impl FileReader {
    /// Reads a .amem file and returns the full graph.
    pub fn read(path: &Path) -> Result<CognitiveGraph, GraphError>;
}
```

**Example:**

```rust
use agentic_memory::io::FileReader;
use std::path::Path;

let graph = FileReader::read(Path::new("agent.amem"))?;
println!("Loaded {} nodes", graph.node_count());
```

### MmapReader

Memory-mapped read access. Does not load the entire file into memory -- pages are loaded on demand by the OS. Ideal for large brain files or read-heavy workloads.

```rust
use agentic_memory::io::MmapReader;

impl MmapReader {
    /// Opens a .amem file with memory-mapped I/O.
    pub fn open(path: &Path) -> Result<Self, GraphError>;

    /// Returns a reference to the node with the given ID.
    pub fn node(&self, id: NodeId) -> Option<CognitiveNodeRef<'_>>;

    /// Returns the total number of nodes.
    pub fn node_count(&self) -> usize;

    /// Returns the total number of edges.
    pub fn edge_count(&self) -> usize;

    /// Returns edges for a given node.
    pub fn edges(&self, id: NodeId, direction: Direction) -> Vec<CognitiveEdgeRef<'_>>;
}
```

**Example:**

```rust
use agentic_memory::io::MmapReader;
use std::path::Path;

let reader = MmapReader::open(Path::new("large_brain.amem"))?;
if let Some(node) = reader.node(NodeId(42)) {
    println!("Node 42: {}", node.content());
}
```

---

## QueryEngine

Provides indexed query operations on top of a `CognitiveGraph` or `MmapReader`.

```rust
use agentic_memory::query::QueryEngine;

impl QueryEngine {
    /// Creates a query engine from an in-memory graph.
    pub fn from_graph(graph: &CognitiveGraph) -> Self;

    /// Creates a query engine from a memory-mapped reader.
    pub fn from_mmap(reader: &MmapReader) -> Self;

    /// Returns all node IDs of the given event type.
    pub fn by_type(&self, event_type: EventType) -> Vec<NodeId>;

    /// Returns all node IDs in the given session.
    pub fn by_session(&self, session: u32) -> Vec<NodeId>;

    /// Performs a BFS traversal from the starting node.
    pub fn traverse(
        &self,
        start: NodeId,
        max_depth: usize,
        edge_filter: Option<&[EdgeType]>,
    ) -> TraversalResult;

    /// Finds the top-k most similar nodes to the given query vector.
    pub fn similarity_search(
        &self,
        query_vector: &[f32; 128],
        top_k: usize,
    ) -> Vec<(NodeId, f32)>;
}
```

**Example:**

```rust
use agentic_memory::query::QueryEngine;

let engine = QueryEngine::from_graph(&graph);

// Find all facts
let facts = engine.by_type(EventType::Fact);

// Similarity search
let results = engine.similarity_search(&query_vec, 10);
for (node_id, score) in results {
    println!("Node {}: score {:.3}", node_id.0, score);
}
```

---

## CLI Reference: `amem`

The `amem` binary provides command-line access to all core operations.

### amem create

Creates a new, empty brain file.

```
amem create <PATH>
```

| Argument | Description |
|----------|-------------|
| `PATH` | Path for the new `.amem` file. |

**Example:**

```bash
amem create project.amem
```

---

### amem add

Adds a cognitive event to a brain file.

```
amem add <PATH> <TYPE> <CONTENT> [OPTIONS]
```

| Argument / Flag | Description |
|-----------------|-------------|
| `PATH` | Path to the `.amem` file. |
| `TYPE` | Event type: `fact`, `decision`, `inference`, `correction`, `skill`, `episode`. |
| `CONTENT` | The text content of the event (quoted string). |
| `--confidence <FLOAT>` | Confidence score, 0.0 to 1.0. Default: 1.0. |
| `--session <INT>` | Session ID. Default: auto-assigned. |
| `--meta <KEY=VALUE>` | Metadata key-value pair. Repeatable. |

**Example:**

```bash
amem add project.amem fact "Rust 1.75 supports async traits" --confidence 0.95 --meta source=docs
```

---

### amem link

Creates an edge between two events.

```
amem link <PATH> <SOURCE> <TARGET> <TYPE> [OPTIONS]
```

| Argument / Flag | Description |
|-----------------|-------------|
| `PATH` | Path to the `.amem` file. |
| `SOURCE` | Source node ID (integer). |
| `TARGET` | Target node ID (integer). |
| `TYPE` | Edge type: `caused_by`, `supports`, `contradicts`, `supersedes`, `related_to`, `part_of`, `temporal_next`. |
| `--weight <FLOAT>` | Edge weight, 0.0 to 1.0. Default: 1.0. |

**Example:**

```bash
amem link project.amem 0 1 supports --weight 0.9
```

---

### amem info

Displays summary information about a brain file.

```
amem info <PATH> [OPTIONS]
```

| Argument / Flag | Description |
|-----------------|-------------|
| `PATH` | Path to the `.amem` file. |
| `--sessions` | Show per-session breakdown. |
| `--json` | Output as JSON. |

**Example:**

```bash
amem info project.amem --sessions
```

---

### amem traverse

Traverses the graph from a starting node and prints the results.

```
amem traverse <PATH> <START_ID> [OPTIONS]
```

| Argument / Flag | Description |
|-----------------|-------------|
| `PATH` | Path to the `.amem` file. |
| `START_ID` | Starting node ID. |
| `--depth <INT>` | Maximum traversal depth. Default: 3. |
| `--edge-type <TYPE>` | Filter to specific edge types. Repeatable. |
| `--json` | Output as JSON. |

**Example:**

```bash
amem traverse project.amem 0 --depth 5 --edge-type supports --edge-type caused_by
```

---

### amem query

Queries events in the brain by type, session, or similarity search.

```
amem query <PATH> [OPTIONS]
```

| Flag | Description |
|------|-------------|
| `--type <TYPE>` | Filter by event type. |
| `--session <INT>` | Filter by session ID. |
| `--search <TEXT>` | Semantic similarity search query. |
| `--top-k <INT>` | Number of results for similarity search. Default: 10. |
| `--min-confidence <FLOAT>` | Minimum confidence threshold. |
| `--json` | Output as JSON. |

**Example:**

```bash
amem query project.amem --type fact --min-confidence 0.8
amem query project.amem --search "database performance" --top-k 5
```

---

### amem mcp-serve

Starts an MCP (Model Context Protocol) server that exposes the brain as a tool for AI coding assistants.

```
amem mcp-serve <PATH> [OPTIONS]
```

| Argument / Flag | Description |
|-----------------|-------------|
| `PATH` | Path to the `.amem` file. |
| `--port <INT>` | Port to listen on. Default: 3100. |
| `--host <ADDR>` | Host address to bind. Default: `127.0.0.1`. |
| `--read-only` | Serve in read-only mode (no writes allowed). |

**Example:**

```bash
amem mcp-serve project.amem --port 3100
```

This exposes the brain as MCP tools that Claude Code, Cursor, Windsurf, and other MCP-compatible editors can connect to. See the [Integration Guide](integration-guide.md) for configuration details.
