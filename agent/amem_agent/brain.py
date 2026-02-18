"""Python wrapper around the ``amem`` CLI binary.

This is the **only** module that communicates with AgenticMemory.  Every
public method maps to a single ``amem`` CLI command, executed as a subprocess.
The design is intentional -- it proves the CLI works as a real integration
surface and keeps the Python layer as thin as possible.
"""

from __future__ import annotations

import json
import logging
import os
import re
import subprocess
from dataclasses import dataclass
from pathlib import Path

logger = logging.getLogger(__name__)

# Regex for parsing the node ID from ``amem add`` text output.
# Example output: "Added node 42 (fact) to brain.amem"
_NODE_ID_RE = re.compile(r"Added node (\d+)")

# Default subprocess timeout in seconds.
_DEFAULT_TIMEOUT = 30


# ---------------------------------------------------------------------------
# Data classes
# ---------------------------------------------------------------------------

@dataclass
class BrainInfo:
    """Summary statistics about a ``.amem`` brain file."""

    version: int
    dimension: int
    node_count: int
    edge_count: int
    session_count: int
    facts: int
    decisions: int
    inferences: int
    corrections: int
    skills: int
    episodes: int
    file_size_bytes: int


# ---------------------------------------------------------------------------
# Exception
# ---------------------------------------------------------------------------

class BrainError(Exception):
    """Raised when an ``amem`` CLI command fails."""

    def __init__(self, command: str, stderr: str, returncode: int) -> None:
        self.command = command
        self.stderr = stderr
        self.returncode = returncode
        super().__init__(
            f"amem command failed (exit {returncode}): {command}\n{stderr}"
        )


# ---------------------------------------------------------------------------
# Brain class
# ---------------------------------------------------------------------------

class Brain:
    """Interface to an AgenticMemory ``.amem`` brain file via the ``amem`` CLI.

    Every method translates to exactly one CLI invocation.  JSON output is
    obtained via the global ``--format json`` flag, which must appear
    **before** the subcommand.

    Args:
        brain_path: Filesystem path to the ``.amem`` file.  Created
            automatically by :meth:`ensure_exists` if absent.
        amem_binary: Path (or name on ``$PATH``) of the ``amem`` binary.
        timeout: Subprocess timeout in seconds.
    """

    def __init__(
        self,
        brain_path: str,
        amem_binary: str = "amem",
        timeout: int = _DEFAULT_TIMEOUT,
    ) -> None:
        self.brain_path = str(Path(brain_path).expanduser())
        self.amem_binary = amem_binary
        self.timeout = timeout

        # Verify the binary exists and is executable.
        self._verify_binary()

    # ------------------------------------------------------------------
    # Binary verification
    # ------------------------------------------------------------------

    def _verify_binary(self) -> None:
        """Check that the ``amem`` binary exists and is callable.

        Raises:
            BrainError: If the binary is missing or not executable.
        """
        binary_path = Path(self.amem_binary)

        # If an absolute or relative path was given, check the filesystem.
        if os.sep in self.amem_binary or self.amem_binary.startswith("."):
            if not binary_path.is_file():
                raise BrainError(
                    command="(init)",
                    stderr=(
                        f"amem binary not found at '{self.amem_binary}'. "
                        "Install AgenticMemory core first."
                    ),
                    returncode=-1,
                )
            if not os.access(str(binary_path), os.X_OK):
                raise BrainError(
                    command="(init)",
                    stderr=(
                        f"amem binary at '{self.amem_binary}' is not executable."
                    ),
                    returncode=-1,
                )
            return

        # If only a name was given, check $PATH via ``which``.
        try:
            subprocess.run(
                ["which", self.amem_binary],
                capture_output=True,
                text=True,
                check=True,
                timeout=5,
            )
        except (subprocess.CalledProcessError, FileNotFoundError):
            raise BrainError(
                command="(init)",
                stderr=(
                    f"amem binary not found at '{self.amem_binary}'. "
                    "Install AgenticMemory core first."
                ),
                returncode=-1,
            )

    # ------------------------------------------------------------------
    # Lifecycle
    # ------------------------------------------------------------------

    def ensure_exists(self) -> None:
        """Create the brain file if it does not already exist.

        Safe to call repeatedly -- it is a no-op when the file is present.
        Also creates parent directories as needed.
        """
        path = Path(self.brain_path)
        if path.is_file():
            return

        # Ensure the parent directory exists.
        path.parent.mkdir(parents=True, exist_ok=True)

        self._run(["create", self.brain_path])
        logger.info("Created brain file: %s", self.brain_path)

    def info(self) -> BrainInfo:
        """Retrieve summary statistics for the brain file.

        Returns:
            A :class:`BrainInfo` populated from the CLI's JSON output.

        Raises:
            BrainError: If the CLI command fails or the output is not valid
                JSON.
        """
        data = self._run_json(["info", self.brain_path])
        node_types = data.get("node_types", {})
        return BrainInfo(
            version=data.get("version", 0),
            dimension=data.get("dimension", 0),
            node_count=data.get("nodes", 0),
            edge_count=data.get("edges", 0),
            session_count=data.get("sessions", 0),
            facts=node_types.get("facts", 0),
            decisions=node_types.get("decisions", 0),
            inferences=node_types.get("inferences", 0),
            corrections=node_types.get("corrections", 0),
            skills=node_types.get("skills", 0),
            episodes=node_types.get("episodes", 0),
            file_size_bytes=data.get("file_size", 0),
        )

    # ------------------------------------------------------------------
    # Write operations
    # ------------------------------------------------------------------

    def add_fact(
        self, content: str, session_id: int, confidence: float = 1.0
    ) -> int:
        """Add a *fact* node to the brain.

        Args:
            content: Textual content of the fact.
            session_id: Session that produced this fact.
            confidence: Confidence score (0.0 -- 1.0).

        Returns:
            The node ID assigned by the engine.
        """
        return self._add_node("fact", content, session_id, confidence)

    def add_decision(
        self, content: str, session_id: int, confidence: float = 1.0
    ) -> int:
        """Add a *decision* node to the brain.

        Args:
            content: Textual content of the decision.
            session_id: Session that produced this decision.
            confidence: Confidence score (0.0 -- 1.0).

        Returns:
            The node ID assigned by the engine.
        """
        return self._add_node("decision", content, session_id, confidence)

    def add_inference(
        self, content: str, session_id: int, confidence: float = 1.0
    ) -> int:
        """Add an *inference* node to the brain.

        Args:
            content: Textual content of the inference.
            session_id: Session that produced this inference.
            confidence: Confidence score (0.0 -- 1.0).

        Returns:
            The node ID assigned by the engine.
        """
        return self._add_node("inference", content, session_id, confidence)

    def add_correction(
        self, content: str, session_id: int, supersedes_id: int
    ) -> int:
        """Add a *correction* node that supersedes an existing node.

        Args:
            content: The corrected information.
            session_id: Session that produced this correction.
            supersedes_id: Node ID of the node being corrected.

        Returns:
            The node ID assigned by the engine.
        """
        output = self._run([
            "add", self.brain_path, "correction", content,
            "--session", str(session_id),
            "--supersedes", str(supersedes_id),
        ])
        return self._parse_node_id(output)

    def add_skill(
        self, content: str, session_id: int, confidence: float = 1.0
    ) -> int:
        """Add a *skill* node to the brain.

        Args:
            content: Textual content of the skill.
            session_id: Session that produced this skill.
            confidence: Confidence score (0.0 -- 1.0).

        Returns:
            The node ID assigned by the engine.
        """
        return self._add_node("skill", content, session_id, confidence)

    def add_episode(self, content: str, session_id: int) -> int:
        """Add an *episode* summary node to the brain.

        Args:
            content: Textual summary of the episode.
            session_id: Session that this episode belongs to.

        Returns:
            The node ID assigned by the engine.
        """
        output = self._run([
            "add", self.brain_path, "episode", content,
            "--session", str(session_id),
        ])
        return self._parse_node_id(output)

    def link(
        self,
        source_id: int,
        target_id: int,
        edge_type: str,
        weight: float = 1.0,
    ) -> None:
        """Create a directed edge between two nodes.

        Args:
            source_id: Source node ID.
            target_id: Target node ID.
            edge_type: One of ``caused_by``, ``supports``, ``contradicts``,
                ``supersedes``, ``related_to``, ``part_of``, ``temporal_next``.
            weight: Edge weight (0.0 -- 1.0).
        """
        self._run([
            "link", self.brain_path,
            str(source_id), str(target_id), edge_type,
            "--weight", str(weight),
        ])

    # ------------------------------------------------------------------
    # Read operations
    # ------------------------------------------------------------------

    def get_node(self, node_id: int) -> dict:
        """Retrieve a single node by its ID.

        Args:
            node_id: The node ID to fetch.

        Returns:
            A dictionary with the node's fields (``id``, ``type``,
            ``content``, ``confidence``, ``session_id``, etc.).
        """
        data = self._run_json(["get", self.brain_path, str(node_id)])
        return data

    def search(
        self,
        event_types: list[str] | None = None,
        session_ids: list[int] | None = None,
        min_confidence: float | None = None,
        sort: str = "recent",
        limit: int = 20,
    ) -> list[dict]:
        """Find nodes matching the given conditions.

        Args:
            event_types: Filter to these event types (e.g. ``["fact",
                "decision"]``).
            session_ids: Filter to these session IDs.
            min_confidence: Minimum confidence threshold.
            sort: Sort order -- ``recent``, ``confidence``, ``accessed``,
                or ``importance``.
            limit: Maximum number of results.

        Returns:
            A list of matching node dictionaries.
        """
        cmd: list[str] = ["search", self.brain_path]

        if event_types:
            cmd.extend(["--event-types", ",".join(event_types)])
        if session_ids:
            cmd.extend(["--session", ",".join(str(s) for s in session_ids)])
        if min_confidence is not None:
            cmd.extend(["--min-confidence", str(min_confidence)])
        cmd.extend(["--sort", sort])
        cmd.extend(["--limit", str(limit)])

        data = self._run_json(cmd)
        if isinstance(data, list):
            return data
        # Defensive: if the CLI returns a wrapper object, try to extract a list.
        return data.get("results", data.get("nodes", []))

    def traverse(
        self,
        start_id: int,
        edge_types: list[str] | None = None,
        direction: str = "backward",
        max_depth: int = 5,
        max_results: int = 50,
    ) -> list[dict]:
        """Traverse the graph from a starting node.

        Args:
            start_id: Node ID to start the traversal from.
            edge_types: Only follow these edge types.
            direction: ``forward``, ``backward``, or ``both``.
            max_depth: Maximum traversal depth.
            max_results: Maximum nodes to return.

        Returns:
            A list of node dictionaries encountered during traversal.
        """
        cmd: list[str] = ["traverse", self.brain_path, str(start_id)]

        if edge_types:
            cmd.extend(["--edge-types", ",".join(edge_types)])
        cmd.extend(["--direction", direction])
        cmd.extend(["--max-depth", str(max_depth)])
        cmd.extend(["--max-results", str(max_results)])

        data = self._run_json(cmd)
        if isinstance(data, list):
            return data
        return data.get("nodes", [])

    def impact(self, node_id: int, max_depth: int = 10) -> dict:
        """Run causal impact analysis on a node.

        Args:
            node_id: The node to analyse.
            max_depth: Maximum traversal depth for impact calculation.

        Returns:
            A dictionary with keys such as ``root_id``,
            ``total_dependents``, ``direct_dependents``,
            ``affected_decisions``, and ``dependents``.
        """
        data = self._run_json([
            "impact", self.brain_path, str(node_id),
            "--max-depth", str(max_depth),
        ])
        return data

    def resolve(self, node_id: int) -> dict:
        """Follow the SUPERSEDES chain to the latest version of a node.

        Args:
            node_id: The (possibly stale) node ID.

        Returns:
            A dictionary describing the resolved node, with
            ``original_id``, ``resolved_id``, ``type``, and ``content``.
        """
        data = self._run_json(["resolve", self.brain_path, str(node_id)])
        return data

    def get_sessions(self, limit: int = 20) -> list[dict]:
        """List all sessions stored in the brain.

        Args:
            limit: Maximum number of sessions to return.

        Returns:
            A list of session dictionaries (``session_id``, ``node_count``).
        """
        data = self._run_json([
            "sessions", self.brain_path,
            "--limit", str(limit),
        ])
        if isinstance(data, list):
            return data
        return data.get("sessions", [])

    # ------------------------------------------------------------------
    # Convenience read methods
    # ------------------------------------------------------------------

    def get_recent_facts(self, limit: int = 20) -> list[dict]:
        """Return the most recent *fact* nodes.

        Args:
            limit: Maximum number of results.

        Returns:
            A list of fact node dictionaries, most recent first.
        """
        return self.search(event_types=["fact"], sort="recent", limit=limit)

    def get_recent_decisions(self, limit: int = 10) -> list[dict]:
        """Return the most recent *decision* nodes.

        Args:
            limit: Maximum number of results.

        Returns:
            A list of decision node dictionaries, most recent first.
        """
        return self.search(event_types=["decision"], sort="recent", limit=limit)

    def get_session_nodes(self, session_id: int) -> list[dict]:
        """Return all nodes belonging to a specific session.

        Args:
            session_id: The session to query.

        Returns:
            A list of node dictionaries for that session.
        """
        return self.search(session_ids=[session_id], limit=1000)

    # ------------------------------------------------------------------
    # Stats
    # ------------------------------------------------------------------

    def stats(self) -> dict:
        """Retrieve detailed graph statistics.

        Returns:
            A dictionary with keys such as ``nodes``, ``edges``,
            ``sessions``, ``file_size``, and type breakdowns.
        """
        data = self._run_json(["stats", self.brain_path])
        return data

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

    def _add_node(
        self,
        event_type: str,
        content: str,
        session_id: int,
        confidence: float,
    ) -> int:
        """Shared implementation for the ``add_*`` methods (except correction
        and episode, which have different flag sets).

        Args:
            event_type: The event type string (e.g. ``"fact"``).
            content: Textual content of the node.
            session_id: Session ID.
            confidence: Confidence score.

        Returns:
            The newly assigned node ID.
        """
        output = self._run([
            "add", self.brain_path, event_type, content,
            "--session", str(session_id),
            "--confidence", str(confidence),
        ])
        return self._parse_node_id(output)

    @staticmethod
    def _parse_node_id(output: str) -> int:
        """Extract the node ID from ``amem add`` text output.

        Args:
            output: The stdout from an ``amem add`` invocation.

        Returns:
            The integer node ID.

        Raises:
            BrainError: If the output does not contain a parseable node ID.
        """
        match = _NODE_ID_RE.search(output)
        if match:
            return int(match.group(1))
        raise BrainError(
            command="add",
            stderr=f"Could not parse node ID from output: {output!r}",
            returncode=-1,
        )

    def _run(self, args: list[str]) -> str:
        """Execute an ``amem`` CLI command and return its ``stdout``.

        The global ``--format`` flag is **not** added here; callers that need
        JSON output should use :meth:`_run_json` instead.

        Args:
            args: Arguments to pass after the binary name.

        Returns:
            The captured standard output as a string.

        Raises:
            BrainError: If the subprocess exits with a non-zero code or
                times out.
        """
        full_cmd = [self.amem_binary] + args
        cmd_str = " ".join(full_cmd)
        logger.debug("Running: %s", cmd_str)

        try:
            result = subprocess.run(
                full_cmd,
                capture_output=True,
                text=True,
                timeout=self.timeout,
            )
        except subprocess.TimeoutExpired:
            raise BrainError(
                command=cmd_str,
                stderr=f"Command timed out after {self.timeout}s",
                returncode=-1,
            )
        except FileNotFoundError:
            raise BrainError(
                command=cmd_str,
                stderr=(
                    f"amem binary not found at '{self.amem_binary}'. "
                    "Install AgenticMemory core first."
                ),
                returncode=-1,
            )

        if result.returncode != 0:
            raise BrainError(
                command=cmd_str,
                stderr=result.stderr.strip(),
                returncode=result.returncode,
            )

        return result.stdout.strip()

    def _run_json(self, args: list[str]) -> dict | list:
        """Execute an ``amem`` CLI command with ``--format json`` and parse
        the output.

        The ``--format json`` flag is inserted as a global option, appearing
        **before** the subcommand (which should be the first element of
        *args*).

        Args:
            args: Arguments to pass after the binary name.  The first
                element should be the subcommand (e.g. ``"info"``).

        Returns:
            The parsed JSON output (a ``dict`` or ``list``).

        Raises:
            BrainError: If the subprocess fails or the output is not valid
                JSON.
        """
        # Insert --format json before the subcommand.
        json_args = ["--format", "json"] + args
        raw = self._run(json_args)

        try:
            return json.loads(raw)
        except json.JSONDecodeError as exc:
            raise BrainError(
                command=" ".join([self.amem_binary] + json_args),
                stderr=(
                    f"Failed to parse JSON output: {exc}\n"
                    f"Raw output: {raw!r}"
                ),
                returncode=-1,
            )
