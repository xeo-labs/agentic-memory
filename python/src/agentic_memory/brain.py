"""The Brain class — primary interface to AgenticMemory.

The Brain class provides a Pythonic API for creating, reading, writing,
and querying cognitive event graphs stored in the AgenticMemory binary
format (.amem files).

The core Brain class has zero external dependencies — it uses only the
Python standard library and communicates with the AgenticMemory engine
via the ``amem`` CLI binary.
"""

from __future__ import annotations

import logging
from pathlib import Path
from typing import Optional

from agentic_memory.cli_bridge import (
    find_amem_binary,
    parse_node_id,
    run_command,
    run_command_json,
    DEFAULT_TIMEOUT,
)
from agentic_memory.errors import (
    BrainNotFoundError,
    CLIError,
    NodeNotFoundError,
    ValidationError,
)
from agentic_memory.models import (
    BrainInfo,
    Edge,
    EdgeType,
    Event,
    EventType,
    ImpactResult,
    SessionInfo,
    TraversalResult,
    parse_brain_info,
    parse_event,
    parse_impact,
    parse_session_info,
    parse_traversal,
)

logger = logging.getLogger(__name__)


class Brain:
    """Interface to an AgenticMemory .amem brain file.

    The Brain class provides a Pythonic API for creating, reading, writing,
    and querying cognitive event graphs stored in the AgenticMemory binary
    format (.amem files).

    The core Brain class has zero external dependencies — it uses only the
    Python standard library and communicates with the AgenticMemory engine
    via the ``amem`` CLI binary.

    Args:
        path: Path to the .amem brain file. Created automatically if it
            doesn't exist.
        amem_binary: Path to the amem CLI binary. If None, searches PATH
            and common install locations.
        auto_create: If True (default), create the brain file on first
            write operation if it doesn't exist.

    Example:
        >>> brain = Brain("my_agent.amem")
        >>> brain.add_fact("User prefers Rust", session=1, confidence=0.95)
        >>> brain.facts(limit=5)
        [Event(id=0, type=EventType.FACT, content='User prefers Rust', ...)]

    Note:
        Brain is not thread-safe. Use separate Brain instances for
        concurrent access to the same file.
    """

    def __init__(
        self,
        path: str | Path,
        amem_binary: str | Path | None = None,
        auto_create: bool = True,
    ) -> None:
        self._path = Path(path).expanduser().resolve()
        self._auto_create = auto_create
        self._binary = find_amem_binary(amem_binary)
        self._timeout = DEFAULT_TIMEOUT

    # ================================================================
    # PROPERTIES
    # ================================================================

    @property
    def path(self) -> Path:
        """The path to the .amem brain file."""
        return self._path

    @property
    def exists(self) -> bool:
        """Whether the brain file exists on disk."""
        return self._path.is_file()

    # ================================================================
    # LIFECYCLE
    # ================================================================

    def create(self, dimension: int = 128) -> None:
        """Create a new empty brain file.

        Args:
            dimension: Feature vector dimensionality (default: 128).

        Raises:
            AmemError: If the file already exists.
        """
        self._path.parent.mkdir(parents=True, exist_ok=True)
        self._cli("create", str(self._path))
        logger.info("Created brain file: %s", self._path)

    def info(self) -> BrainInfo:
        """Get brain statistics.

        Returns:
            BrainInfo with node/edge counts, session count, type breakdown.

        Raises:
            BrainNotFoundError: If the brain file doesn't exist.
        """
        if not self.exists:
            raise BrainNotFoundError(str(self._path))
        data = self._cli_json("info", str(self._path))
        if not isinstance(data, dict):
            data = {}
        return parse_brain_info(data, str(self._path))

    # ================================================================
    # WRITE OPERATIONS — Adding knowledge
    # ================================================================

    def add_fact(
        self,
        content: str,
        session: int,
        confidence: float = 1.0,
    ) -> int:
        """Add a fact to the brain.

        A fact is something the agent learned about the world or the user.

        Args:
            content: The fact text (e.g., "User's name is Marcus").
            session: Session ID this fact was learned in.
            confidence: Certainty level, 0.0 to 1.0 (default: 1.0).

        Returns:
            The assigned node ID.

        Example:
            >>> brain.add_fact("User lives in Toronto", session=1)
            42
        """
        return self._add_node("fact", content, session, confidence)

    def add_decision(
        self,
        content: str,
        session: int,
        confidence: float = 1.0,
    ) -> int:
        """Add a decision to the brain.

        A decision records a choice the agent made and its reasoning.

        Args:
            content: The decision text (e.g., "Recommended Python because
                the team has no Rust experience").
            session: Session ID this decision was made in.
            confidence: Certainty level, 0.0 to 1.0 (default: 1.0).

        Returns:
            The assigned node ID.
        """
        return self._add_node("decision", content, session, confidence)

    def add_inference(
        self,
        content: str,
        session: int,
        confidence: float = 1.0,
    ) -> int:
        """Add an inference to the brain.

        An inference is a conclusion drawn from multiple facts.

        Args:
            content: The inference text.
            session: Session ID.
            confidence: Certainty level (default: 1.0).

        Returns:
            The assigned node ID.
        """
        return self._add_node("inference", content, session, confidence)

    def add_correction(
        self,
        content: str,
        session: int,
        supersedes: int,
    ) -> int:
        """Add a correction that replaces an existing node.

        Creates a new CORRECTION node with a SUPERSEDES edge to the old node.
        The old node's confidence is reduced to 0.0.

        Args:
            content: The corrected information.
            session: Session ID.
            supersedes: ID of the node being corrected.

        Returns:
            The new correction node ID.

        Example:
            >>> old_id = brain.add_fact("User works at TechCorp", session=1)
            >>> new_id = brain.add_correction("User now works at DataFlow", session=5, supersedes=old_id)
        """
        self._ensure_exists()
        output = self._cli(
            "add", str(self._path), "correction", content,
            "--session", str(session),
            "--supersedes", str(supersedes),
        )
        return parse_node_id(output)

    def add_skill(
        self,
        content: str,
        session: int,
        confidence: float = 1.0,
    ) -> int:
        """Add a learned skill/pattern to the brain.

        A skill is a procedural memory — how to do something for this user.

        Args:
            content: The skill description.
            session: Session ID.
            confidence: Certainty level (default: 1.0).

        Returns:
            The assigned node ID.
        """
        return self._add_node("skill", content, session, confidence)

    def add_episode(
        self,
        content: str,
        session: int,
    ) -> int:
        """Add a session episode summary.

        An episode is a compressed summary of an interaction session.

        Args:
            content: The session summary text.
            session: Session ID this episode covers.

        Returns:
            The assigned node ID.
        """
        self._ensure_exists()
        output = self._cli(
            "add", str(self._path), "episode", content,
            "--session", str(session),
        )
        return parse_node_id(output)

    def link(
        self,
        source: int,
        target: int,
        edge_type: str | EdgeType,
        weight: float = 1.0,
    ) -> None:
        """Create an edge between two nodes.

        Args:
            source: Source node ID.
            target: Target node ID.
            edge_type: Relationship type. Can be an EdgeType enum or string:
                "caused_by", "supports", "contradicts", "supersedes",
                "related_to", "part_of", "temporal_next".
            weight: Relationship strength, 0.0 to 1.0 (default: 1.0).

        Raises:
            NodeNotFoundError: If source or target doesn't exist.
            ValidationError: If source == target (no self-edges).
        """
        if source == target:
            raise ValidationError(f"Self-edges are not allowed: {source} -> {target}")

        edge_type_str = str(edge_type.value) if isinstance(edge_type, EdgeType) else str(edge_type)
        self._cli(
            "link", str(self._path),
            str(source), str(target), edge_type_str,
            "--weight", str(weight),
        )

    # ================================================================
    # READ OPERATIONS — Querying knowledge
    # ================================================================

    def get(self, node_id: int) -> Event:
        """Get a specific node by ID.

        Args:
            node_id: The node ID to retrieve.

        Returns:
            The Event at that ID.

        Raises:
            NodeNotFoundError: If the node doesn't exist.
        """
        if not self.exists:
            raise BrainNotFoundError(str(self._path))
        try:
            data = self._cli_json("get", str(self._path), str(node_id))
        except CLIError as e:
            if "not found" in e.stderr.lower() or "does not exist" in e.stderr.lower():
                raise NodeNotFoundError(node_id) from e
            raise
        if not isinstance(data, dict):
            raise NodeNotFoundError(node_id)
        return parse_event(data)

    def search(
        self,
        types: list[str | EventType] | None = None,
        sessions: list[int] | None = None,
        min_confidence: float | None = None,
        max_confidence: float | None = None,
        after: str | None = None,
        before: str | None = None,
        sort: str = "recent",
        limit: int = 20,
    ) -> list[Event]:
        """Search for nodes matching conditions.

        Args:
            types: Filter by event type(s). None = all types.
            sessions: Filter by session ID(s). None = all sessions.
            min_confidence: Minimum confidence (inclusive).
            max_confidence: Maximum confidence (inclusive).
            after: Created after this timestamp (ISO 8601 or Unix micros).
            before: Created before this timestamp.
            sort: Sort order — "recent", "confidence", "accessed", "importance".
            limit: Maximum results (default: 20).

        Returns:
            List of matching Events, sorted as specified.
        """
        if not self.exists:
            raise BrainNotFoundError(str(self._path))

        cmd: list[str] = ["search", str(self._path)]

        if types:
            type_strs = [str(t.value) if isinstance(t, EventType) else str(t) for t in types]
            cmd.extend(["--event-types", ",".join(type_strs)])
        if sessions:
            cmd.extend(["--session", ",".join(str(s) for s in sessions)])
        if min_confidence is not None:
            cmd.extend(["--min-confidence", str(min_confidence)])
        if max_confidence is not None:
            cmd.extend(["--max-confidence", str(max_confidence)])
        if after is not None:
            cmd.extend(["--after", after])
        if before is not None:
            cmd.extend(["--before", before])
        cmd.extend(["--sort", sort])
        cmd.extend(["--limit", str(limit)])

        data = self._cli_json(*cmd)

        results: list[dict] = []  # type: ignore[type-arg]
        if isinstance(data, list):
            results = data
        elif isinstance(data, dict):
            results = data.get("results", data.get("nodes", []))

        return [parse_event(r) for r in results]

    def facts(self, limit: int = 20, min_confidence: float | None = None) -> list[Event]:
        """Get recent facts. Convenience wrapper around search().

        Args:
            limit: Maximum results.
            min_confidence: Minimum confidence filter.

        Returns:
            List of fact Events, most recent first.
        """
        return self.search(types=["fact"], limit=limit, min_confidence=min_confidence)

    def decisions(self, limit: int = 10) -> list[Event]:
        """Get recent decisions. Convenience wrapper around search()."""
        return self.search(types=["decision"], limit=limit)

    def corrections(self, limit: int = 10) -> list[Event]:
        """Get recent corrections."""
        return self.search(types=["correction"], limit=limit)

    def skills(self, limit: int = 10) -> list[Event]:
        """Get learned skills."""
        return self.search(types=["skill"], limit=limit)

    # ================================================================
    # GRAPH OPERATIONS — Navigating the knowledge graph
    # ================================================================

    def traverse(
        self,
        start: int,
        edges: list[str | EdgeType] | None = None,
        direction: str = "backward",
        max_depth: int = 5,
        max_results: int = 50,
        min_confidence: float = 0.0,
    ) -> TraversalResult:
        """Traverse the graph from a starting node.

        Walk the knowledge graph following specific edge types. Use this to
        reconstruct reasoning chains ("why did I decide this?").

        Args:
            start: Starting node ID.
            edges: Edge types to follow. None = all types.
            direction: "forward" (outgoing), "backward" (incoming), "both".
            max_depth: Maximum hops from start.
            max_results: Maximum nodes to visit.
            min_confidence: Skip nodes below this confidence.

        Returns:
            TraversalResult with visited nodes, edges, and depths.

        Example:
            >>> result = brain.traverse(decision_id, edges=["caused_by"], direction="backward")
            >>> for node_id in result.visited:
            ...     print(brain.get(node_id).content)
        """
        if not self.exists:
            raise BrainNotFoundError(str(self._path))

        cmd: list[str] = ["traverse", str(self._path), str(start)]

        if edges:
            edge_strs = [str(e.value) if isinstance(e, EdgeType) else str(e) for e in edges]
            cmd.extend(["--edge-types", ",".join(edge_strs)])
        cmd.extend(["--direction", direction])
        cmd.extend(["--max-depth", str(max_depth)])
        cmd.extend(["--max-results", str(max_results)])

        data = self._cli_json(*cmd)
        if not isinstance(data, dict):
            data = {"nodes": data if isinstance(data, list) else []}
        return parse_traversal(data)

    def impact(self, node_id: int, max_depth: int = 10) -> ImpactResult:
        """Causal impact analysis: what depends on this node?

        Find every decision and inference that is built on top of a
        given fact or inference. Answers: "what breaks if this is wrong?"

        Args:
            node_id: The node to analyze.
            max_depth: Maximum traversal depth.

        Returns:
            ImpactResult with dependents, affected decisions, and
            affected inferences counts.
        """
        if not self.exists:
            raise BrainNotFoundError(str(self._path))

        data = self._cli_json(
            "impact", str(self._path), str(node_id),
            "--max-depth", str(max_depth),
        )
        if not isinstance(data, dict):
            data = {}
        return parse_impact(data)

    def resolve(self, node_id: int) -> Event:
        """Follow the SUPERSEDES chain to get the latest version.

        If a fact was corrected, this returns the correction (not the
        original). If it was corrected multiple times, returns the most
        recent correction in the chain.

        Args:
            node_id: The node ID (may be outdated).

        Returns:
            The latest Event in the supersedes chain.

        Example:
            >>> old = brain.add_fact("Works at TechCorp", session=1)
            >>> new = brain.add_correction("Works at DataFlow", session=5, supersedes=old)
            >>> brain.resolve(old).content
            'Works at DataFlow'
        """
        if not self.exists:
            raise BrainNotFoundError(str(self._path))

        data = self._cli_json("resolve", str(self._path), str(node_id))
        if isinstance(data, dict):
            # CLI may return resolved_id or the full node
            resolved_id = data.get("resolved_id", data.get("id", node_id))
            if resolved_id != node_id and "content" not in data:
                # Need to fetch the full node
                return self.get(resolved_id)
            if "content" in data:
                return parse_event(data)
        return self.get(node_id)

    def context(self, node_id: int, depth: int = 1) -> list[Event]:
        """Get a node and its neighborhood.

        Returns the node and all directly connected nodes up to the
        specified depth. Useful for understanding the full context
        around a specific memory.

        Args:
            node_id: Center node.
            depth: How many hops out to include.

        Returns:
            List of Events in the neighborhood (center node first).
        """
        result = self.traverse(
            start=node_id,
            direction="both",
            max_depth=depth,
        )
        events: list[Event] = []
        for nid in result.visited:
            try:
                events.append(self.get(nid))
            except NodeNotFoundError:
                continue
        return events

    # ================================================================
    # SESSION OPERATIONS
    # ================================================================

    def sessions(self, limit: int = 20) -> list[SessionInfo]:
        """List sessions in the brain.

        Returns:
            List of SessionInfo with session ID and node count.
        """
        if not self.exists:
            raise BrainNotFoundError(str(self._path))

        data = self._cli_json(
            "sessions", str(self._path),
            "--limit", str(limit),
        )

        results: list[dict] = []  # type: ignore[type-arg]
        if isinstance(data, list):
            results = data
        elif isinstance(data, dict):
            results = data.get("sessions", [])

        return [parse_session_info(s) for s in results]

    def session_events(self, session: int) -> list[Event]:
        """Get all events from a specific session.

        Args:
            session: Session ID.

        Returns:
            List of Events from that session.
        """
        return self.search(sessions=[session], limit=1000)

    # ================================================================
    # STATISTICS
    # ================================================================

    def stats(self) -> dict:  # type: ignore[type-arg]
        """Detailed graph statistics.

        Returns:
            Dictionary with node/edge counts, distributions, content
            block size, compression ratio, etc.
        """
        if not self.exists:
            raise BrainNotFoundError(str(self._path))

        data = self._cli_json("stats", str(self._path))
        if isinstance(data, dict):
            return data
        return {}

    # ================================================================
    # INTERNAL — Not part of public API
    # ================================================================

    def _ensure_exists(self) -> None:
        """Create the brain file if it doesn't exist and auto_create is True."""
        if self.exists:
            return
        if self._auto_create:
            self.create()
        else:
            raise BrainNotFoundError(str(self._path))

    def _cli(self, *args: str) -> str:
        """Run an amem CLI command. Internal use only."""
        return run_command(self._binary, list(args), self._timeout)

    def _cli_json(self, *args: str) -> dict | list:  # type: ignore[type-arg]
        """Run an amem CLI command and parse JSON output. Internal use only."""
        return run_command_json(self._binary, list(args), self._timeout)

    def _add_node(
        self,
        event_type: str,
        content: str,
        session: int,
        confidence: float,
    ) -> int:
        """Shared implementation for the add_* methods."""
        self._ensure_exists()
        output = self._cli(
            "add", str(self._path), event_type, content,
            "--session", str(session),
            "--confidence", str(confidence),
        )
        return parse_node_id(output)
