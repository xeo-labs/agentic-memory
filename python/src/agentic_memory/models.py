"""Typed data models for the AgenticMemory SDK.

All models are frozen dataclasses — immutable after creation. This
prevents accidental mutation and makes them safe to share across
threads.

Enums use ``str, Enum`` so they serialize naturally and can be passed
directly to CLI commands.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum
from typing import Optional


# ===================================================================
# Enums
# ===================================================================

class EventType(str, Enum):
    """Type of cognitive event."""
    FACT = "fact"
    DECISION = "decision"
    INFERENCE = "inference"
    CORRECTION = "correction"
    SKILL = "skill"
    EPISODE = "episode"


class EdgeType(str, Enum):
    """Type of relationship between events."""
    CAUSED_BY = "caused_by"
    SUPPORTS = "supports"
    CONTRADICTS = "contradicts"
    SUPERSEDES = "supersedes"
    RELATED_TO = "related_to"
    PART_OF = "part_of"
    TEMPORAL_NEXT = "temporal_next"


# ===================================================================
# Core Models
# ===================================================================

@dataclass(frozen=True)
class Event:
    """A cognitive event in the brain.

    Attributes:
        id: Unique node identifier.
        type: Event type (fact, decision, inference, etc.).
        content: The text content of this event.
        session: Session ID where this event was created.
        confidence: Certainty level, 0.0 to 1.0.
        access_count: Number of times this event has been accessed.
        decay_score: Current importance decay score.
        created_at: Creation timestamp (ISO 8601 string).
        edges_out: Number of outgoing edges.
        edges_in: Number of incoming edges.
    """
    id: int
    type: EventType
    content: str
    session: int
    confidence: float
    access_count: int = 0
    decay_score: float = 1.0
    created_at: str = ""
    edges_out: int = 0
    edges_in: int = 0


@dataclass(frozen=True)
class Edge:
    """A relationship between two events.

    Attributes:
        source: Source node ID.
        target: Target node ID.
        type: Relationship type.
        weight: Relationship strength, 0.0 to 1.0.
    """
    source: int
    target: int
    type: EdgeType
    weight: float = 1.0


@dataclass(frozen=True)
class BrainInfo:
    """Summary statistics about a brain file.

    Attributes:
        path: Path to the .amem file.
        version: Format version.
        dimension: Feature vector dimensionality.
        node_count: Total nodes.
        edge_count: Total edges.
        session_count: Number of unique sessions.
        file_size: File size in bytes.
        facts: Number of fact nodes.
        decisions: Number of decision nodes.
        inferences: Number of inference nodes.
        corrections: Number of correction nodes.
        skills: Number of skill nodes.
        episodes: Number of episode nodes.
    """
    path: str
    version: int
    dimension: int
    node_count: int
    edge_count: int
    session_count: int
    file_size: int
    facts: int = 0
    decisions: int = 0
    inferences: int = 0
    corrections: int = 0
    skills: int = 0
    episodes: int = 0

    @property
    def is_empty(self) -> bool:
        """Whether the brain has no nodes."""
        return self.node_count == 0


@dataclass(frozen=True)
class SessionInfo:
    """Information about a session in the brain.

    Attributes:
        session_id: The session identifier.
        node_count: Number of nodes in this session.
    """
    session_id: int
    node_count: int


# ===================================================================
# Query Result Models
# ===================================================================

@dataclass(frozen=True)
class TraversalResult:
    """Result of a graph traversal query.

    Attributes:
        visited: Ordered list of visited node IDs (BFS order).
        depths: Mapping of node ID to depth from start.
        start_id: The starting node ID.
        edge_types_followed: Which edge types were followed.
        direction: Traversal direction used.
    """
    visited: list[int]
    depths: dict[int, int]
    start_id: int
    edge_types_followed: list[str]
    direction: str

    @property
    def count(self) -> int:
        """Number of nodes visited."""
        return len(self.visited)

    def at_depth(self, depth: int) -> list[int]:
        """Get all node IDs at a specific depth."""
        return [nid for nid, d in self.depths.items() if d == depth]


@dataclass(frozen=True)
class ImpactResult:
    """Result of causal impact analysis.

    Attributes:
        root_id: The node being analyzed.
        dependents: All nodes that depend on the root.
        affected_decisions: Number of decision nodes affected.
        affected_inferences: Number of inference nodes affected.
        total_dependents: Total number of dependent nodes.
        max_depth: Deepest dependency chain length.
    """
    root_id: int
    dependents: list[int]
    affected_decisions: int
    affected_inferences: int
    total_dependents: int
    max_depth: int

    @property
    def has_dependents(self) -> bool:
        """Whether any nodes depend on this one."""
        return self.total_dependents > 0


# ===================================================================
# Parsing Helpers (internal — used by Brain to convert CLI JSON output)
# ===================================================================

def parse_event(data: dict) -> Event:  # type: ignore[type-arg]
    """Parse a CLI JSON node object into an Event.

    Handles variations in CLI output format (the CLI may use different
    key names or formats across versions).
    """
    return Event(
        id=data.get("id", data.get("node_id", 0)),
        type=EventType(data.get("type", data.get("event_type", "fact"))),
        content=data.get("content", ""),
        session=data.get("session", data.get("session_id", 0)),
        confidence=float(data.get("confidence", 1.0)),
        access_count=int(data.get("access_count", 0)),
        decay_score=float(data.get("decay_score", 1.0)),
        created_at=data.get("created_at", ""),
        edges_out=int(data.get("edges_out", data.get("edge_count", 0))),
        edges_in=int(data.get("edges_in", 0)),
    )


def parse_brain_info(data: dict, path: str) -> BrainInfo:  # type: ignore[type-arg]
    """Parse CLI JSON info output into BrainInfo."""
    types = data.get("node_types", data.get("types", {}))
    return BrainInfo(
        path=path,
        version=data.get("version", 1),
        dimension=data.get("dimension", 128),
        node_count=data.get("node_count", data.get("nodes", 0)),
        edge_count=data.get("edge_count", data.get("edges", 0)),
        session_count=data.get("session_count", data.get("sessions", 0)),
        file_size=data.get("file_size", data.get("file_size_bytes", 0)),
        facts=types.get("facts", types.get("Fact", 0)),
        decisions=types.get("decisions", types.get("Decision", 0)),
        inferences=types.get("inferences", types.get("Inference", 0)),
        corrections=types.get("corrections", types.get("Correction", 0)),
        skills=types.get("skills", types.get("Skill", 0)),
        episodes=types.get("episodes", types.get("Episode", 0)),
    )


def parse_traversal(data: dict) -> TraversalResult:  # type: ignore[type-arg]
    """Parse CLI JSON traverse output into TraversalResult."""
    nodes = data.get("nodes", data.get("visited", []))
    # Build visited list and depths map
    visited: list[int] = []
    depths: dict[int, int] = {}

    if isinstance(nodes, list):
        for item in nodes:
            if isinstance(item, dict):
                nid = item.get("id", item.get("node_id", 0))
                depth = item.get("depth", 0)
                visited.append(nid)
                depths[nid] = depth
            elif isinstance(item, int):
                visited.append(item)
                depths[item] = 0

    return TraversalResult(
        visited=visited,
        depths=depths,
        start_id=data.get("start_id", data.get("start", visited[0] if visited else 0)),
        edge_types_followed=data.get("edge_types", data.get("edge_types_followed", [])),
        direction=data.get("direction", "backward"),
    )


def parse_impact(data: dict) -> ImpactResult:  # type: ignore[type-arg]
    """Parse CLI JSON impact output into ImpactResult."""
    dependents = data.get("dependents", [])
    # Handle both list of ints and list of dicts
    dep_ids: list[int] = []
    for d in dependents:
        if isinstance(d, int):
            dep_ids.append(d)
        elif isinstance(d, dict):
            dep_ids.append(d.get("id", d.get("node_id", 0)))

    return ImpactResult(
        root_id=data.get("root_id", data.get("node_id", 0)),
        dependents=dep_ids,
        affected_decisions=data.get("affected_decisions", 0),
        affected_inferences=data.get("affected_inferences", 0),
        total_dependents=data.get("total_dependents", len(dep_ids)),
        max_depth=data.get("max_depth", 0),
    )


def parse_session_info(data: dict) -> SessionInfo:  # type: ignore[type-arg]
    """Parse a session entry into SessionInfo."""
    return SessionInfo(
        session_id=data.get("session_id", data.get("id", 0)),
        node_count=data.get("node_count", data.get("nodes", 0)),
    )
