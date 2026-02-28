---
status: stable
---

# MCP Tools

AgenticMemory exposes 25 core tools through the MCP protocol via `agentic-memory-mcp`. Additional advanced tools (~100) extend the core set with advanced capabilities.

## Conversation Tools

### `conversation_log`

Log a user prompt and/or agent response into the conversation thread. Entries are automatically linked into the session's temporal chain.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `user_message` | string | No | What the user said or asked |
| `agent_response` | string | No | Summary of the agent's response or action taken |
| `topic` | string | No | Optional topic or category (e.g., `project-setup`, `debugging`) |

At least one of `user_message` or `agent_response` must be provided.

## Memory Tools

### `memory_add`

Add a new cognitive event to the memory graph.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `event_type` | string | Yes | `fact`, `decision`, `inference`, `correction`, `skill`, `episode` |
| `content` | string | Yes | The content of the memory |
| `confidence` | number | No | Confidence level 0.0-1.0 (default: 0.9) |
| `edges` | array | No | Edges to create: `[{"target_id": N, "edge_type": "...", "weight": 1.0}]` |

Edge types: `caused_by`, `derived_from`, `supports`, `contradicts`, `supersedes`, `related_to`, `part_of`, `temporal_next`

**Returns:** `{ "node_id": 42, "event_type": "fact", "edges_created": 1 }`

### `memory_query`

Find memories matching conditions (pattern query).

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `event_types` | array | No | Filter by event types (e.g., `["fact", "decision"]`) |
| `min_confidence` | number | No | Minimum confidence threshold |
| `max_confidence` | number | No | Maximum confidence threshold |
| `session_ids` | array | No | Filter by session IDs |
| `created_after` | integer | No | Created after (Unix microseconds) |
| `created_before` | integer | No | Created before (Unix microseconds) |
| `max_results` | integer | No | Maximum results (default: 20) |
| `sort_by` | string | No | `most_recent`, `highest_confidence`, `most_accessed`, `most_important` (default: `most_recent`) |

### `memory_traverse`

Walk the graph from a starting node, following edges of specified types.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `start_id` | integer | Yes | Starting node ID |
| `edge_types` | array | No | Edge types to follow (default: all types) |
| `direction` | string | No | `forward`, `backward`, `both` (default: `forward`) |
| `max_depth` | integer | No | Maximum traversal depth (default: 5) |
| `max_results` | integer | No | Maximum nodes to return (default: 20) |
| `min_confidence` | number | No | Minimum confidence filter |

### `memory_context`

Get the full context (subgraph) around a node.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `node_id` | integer | Yes | Center node ID |
| `depth` | integer | No | Traversal depth 1-5 (default: 2) |

### `memory_similar`

Find semantically similar memories using vector similarity.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query_text` | string | No | Text query (uses BM25 fallback) |
| `query_vec` | array | No | Embedding vector for cosine similarity |
| `top_k` | integer | No | Maximum results (default: 10) |
| `min_similarity` | number | No | Minimum similarity score (default: 0.5) |
| `event_types` | array | No | Filter by event types |

Either `query_text` or `query_vec` must be provided.

### `memory_correct`

Record a correction to a previous belief. Creates a new node that supersedes the old one.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `old_node_id` | integer | Yes | ID of the node being corrected |
| `new_content` | string | Yes | The correct information |
| `confidence` | number | No | Confidence level (default: 0.95) |
| `reason` | string | No | Explanation for the correction |

### `memory_resolve`

Follow the supersedes chain to get the latest version of a belief.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `node_id` | integer | Yes | Node ID to resolve |

**Returns:** `{ "original_id": 1, "resolved_id": 5, "is_latest": false, "latest": {...} }`

### `memory_causal`

Impact analysis -- find everything that depends on a given node.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `node_id` | integer | Yes | Node ID to analyze |
| `max_depth` | integer | No | Maximum traversal depth (default: 5) |

**Returns:** `{ "root_id": 1, "dependent_count": 3, "affected_decisions": 1, "affected_inferences": 2, "dependents": [...] }`

### `memory_temporal`

Compare knowledge across two time periods.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `range_a` | object | Yes | First time range (see below) |
| `range_b` | object | Yes | Second time range (see below) |

Time range formats:
- `{"type": "time_window", "start": <unix_us>, "end": <unix_us>}`
- `{"type": "session", "session_id": <id>}`
- `{"type": "sessions", "session_ids": [<id>, ...]}`

### `memory_quality`

Evaluate memory reliability: confidence, staleness, orphan nodes, and unsupported decisions.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `low_confidence_threshold` | number | No | Confidence below this is flagged (default: 0.45) |
| `stale_decay_threshold` | number | No | Decay below this is flagged (default: 0.20) |
| `max_examples` | integer | No | Maximum example node IDs per category (default: 20) |

### `memory_stats`

Get statistics about the memory graph. Takes no parameters.

**Returns:** `{ "node_count": 142, "edge_count": 215, "dimension": 128, "session_count": 8, "type_counts": {...}, "file_size_bytes": 12800 }`

## Grounding Tools (Anti-Hallucination)

### `memory_ground`

Verify a claim has memory backing. Returns verified/partial/ungrounded status to prevent hallucination about what was previously remembered.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `claim` | string | Yes | The claim to verify against stored memories |
| `threshold` | number | No | Minimum BM25 score to consider a match (default: 0.3) |

**Returns:** `{ "status": "verified", "claim": "...", "confidence": 0.85, "evidence": [...] }`

### `memory_evidence`

Get detailed evidence for a claim from stored memories. Returns matching memory nodes with full content, timestamps, sessions, and relationships.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | Yes | The query to search evidence for |
| `max_results` | integer | No | Maximum evidence items (default: 10) |

### `memory_suggest`

Find similar memories when a claim does not match exactly. Useful for correcting misremembered facts or finding related knowledge.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | Yes | The query to find suggestions for |
| `limit` | integer | No | Maximum suggestions (default: 5) |

## Workspace Tools

### `memory_workspace_create`

Create a multi-memory workspace for loading and querying multiple `.amem` files simultaneously.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `name` | string | Yes | Name for the workspace |

### `memory_workspace_add`

Add an `.amem` memory file to a workspace.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `workspace_id` | string | Yes | ID of the workspace |
| `path` | string | Yes | Path to the `.amem` file |
| `role` | string | No | `primary`, `secondary`, `reference`, `archive` (default: `primary`) |
| `label` | string | No | Human-readable label for this context |

### `memory_workspace_list`

List all loaded memory contexts in a workspace.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `workspace_id` | string | Yes | ID of the workspace |

### `memory_workspace_query`

Search across all memory contexts in a workspace.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `workspace_id` | string | Yes | ID of the workspace |
| `query` | string | Yes | Text query to search across all contexts |
| `max_per_context` | integer | No | Maximum matches per context (default: 10) |

### `memory_workspace_compare`

Compare how a topic appears across different memory contexts.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `workspace_id` | string | Yes | ID of the workspace |
| `item` | string | Yes | Topic/concept to compare across contexts |
| `max_per_context` | integer | No | Maximum matches per context (default: 5) |

### `memory_workspace_xref`

Cross-reference a topic to find which memory contexts contain it and which do not.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `workspace_id` | string | Yes | ID of the workspace |
| `item` | string | Yes | Topic/concept to cross-reference |

## Session Tools

### `session_start`

Start a new interaction session. Returns context from the previous session to solve the bootstrap problem.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `session_id` | integer | No | Optional explicit session ID |
| `metadata` | object | No | Optional session metadata |

### `session_end`

End a session and optionally create an episode summary node.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `session_id` | integer | No | Session ID (defaults to current) |
| `create_episode` | boolean | No | Create an episode summary node (default: true) |
| `summary` | string | No | Episode summary content |

### `memory_session_resume`

Load context from previous sessions. Call this at the start of every conversation to restore prior context. Returns the last session summary, recent decisions, and key facts.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `limit` | integer | No | Maximum number of recent memories to load (default: 15) |

**Returns:** `{ "current_session": 8, "last_episode": {...}, "recent_decisions": [...], "recent_facts": [...], "total_loaded": 12 }`
