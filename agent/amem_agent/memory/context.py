"""Memory context builder and extract-and-store convenience flow.

This module is responsible for two key operations:

1. **Building memory context** -- querying the brain's memory graph across
   multiple layers (core identity, recent context, decisions, corrections)
   and formatting the results into a markdown string that can be injected
   into the LLM's system prompt.

2. **Extract-and-store** -- the full pipeline that takes a conversation turn,
   extracts cognitive events via the extractor, writes them into the brain,
   resolves inter-memory relationships, and handles corrections.

Memory formation must NEVER crash the agent.  Every public function catches
all exceptions and either returns a safe default or silently logs.
"""

from __future__ import annotations

import logging
from typing import Any, Protocol

from .extractor import (
    ExtractionResult,
    extract_events,
    find_best_match,
    format_existing_memories,
)

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

_MAX_CONTEXT_CHARS: int = 8000
"""Approximate character budget for the memory context block (~2000 tokens)."""

_CONTEXT_HEADER: str = (
    "# Memory Context\n"
    "The following is what I remember from our previous conversations.\n"
)

_EVENT_TYPE_TO_ADD_METHOD: dict[str, str] = {
    "fact": "add_fact",
    "decision": "add_decision",
    "inference": "add_inference",
    "skill": "add_skill",
}
"""Maps event_type strings to the corresponding ``brain.add_*`` method name."""


# ---------------------------------------------------------------------------
# Protocol stubs
# ---------------------------------------------------------------------------

class BrainProtocol(Protocol):
    """Minimal interface the context builder needs from a Brain object."""

    def search(
        self,
        *,
        event_types: list[str] | None = None,
        session_ids: list[int] | None = None,
        min_confidence: float | None = None,
        sort: str = "recent",
        limit: int = 10,
    ) -> list[dict[str, Any]]:
        """Search the memory graph and return matching node dicts."""
        ...

    def add_fact(self, content: str, session_id: int, confidence: float) -> int: ...
    def add_decision(self, content: str, session_id: int, confidence: float) -> int: ...
    def add_inference(self, content: str, session_id: int, confidence: float) -> int: ...
    def add_skill(self, content: str, session_id: int, confidence: float) -> int: ...
    def add_correction(self, content: str, session_id: int, supersedes_id: int) -> int: ...
    def link(self, source_id: int, target_id: int, edge_type: str, weight: float) -> None: ...
    def get_recent_facts(self, limit: int = 10) -> list[dict[str, Any]]: ...


# ---------------------------------------------------------------------------
# Layer helpers
# ---------------------------------------------------------------------------

def _format_memory_section(
    heading: str,
    nodes: list[dict[str, Any]],
) -> str:
    """Render a list of memory nodes as a markdown section.

    Args:
        heading: Markdown heading text (without ``##`` prefix).
        nodes: Node dicts with at least ``content`` and ``confidence``.

    Returns:
        Formatted markdown block, or empty string if *nodes* is empty.
    """
    if not nodes:
        return ""

    lines: list[str] = [f"## {heading}\n"]
    for node in nodes:
        try:
            content = node.get("content", "")
            confidence = node.get("confidence", 0.0)
            pct = int(round(float(confidence) * 100))
            event_type = str(node.get("event_type", "")).upper()
            prefix = f"[{event_type}] " if event_type else ""
            lines.append(f"- {prefix}{content} *(confidence: {pct}%)*")
        except Exception:
            continue
    lines.append("")  # trailing blank line
    return "\n".join(lines)


def _derive_adjacent_session_ids(session_id: int) -> list[int]:
    """Return a list of the current and two preceding session IDs.

    Args:
        session_id: The current session ID (positive integer).

    Returns:
        A deduplicated list of session IDs: ``[current, current-1, current-2]``
        (only positive values, duplicates removed).
    """
    try:
        current = int(session_id)
        ids = [max(current - i, 0) for i in range(3)]
        # Deduplicate while preserving order (handles session 0 and 1).
        seen: set[int] = set()
        unique: list[int] = []
        for sid in ids:
            if sid not in seen:
                seen.add(sid)
                unique.append(sid)
        return unique
    except (ValueError, TypeError):
        return [int(session_id)]


def _truncate(text: str, max_chars: int = _MAX_CONTEXT_CHARS) -> str:
    """Truncate *text* to at most *max_chars*, appending an ellipsis marker."""
    if len(text) <= max_chars:
        return text
    return text[:max_chars].rsplit("\n", 1)[0] + "\n\n*(memory context truncated)*\n"


# ---------------------------------------------------------------------------
# Public API -- build_memory_context
# ---------------------------------------------------------------------------

def build_memory_context(
    brain: BrainProtocol,
    session_id: int,
    user_message: str,
    llm: Any = None,
) -> str:
    """Build a layered memory context string for injection into the system prompt.

    The context is assembled from four memory layers:

    1. **Core Identity** -- high-confidence facts about the user.
    2. **Recent Context** -- memories from the current and adjacent sessions.
    3. **Active Decisions** -- the most recent decisions on record.
    4. **Corrections** -- recently updated or superseded information.

    The result is a markdown-formatted block capped at approximately
    8 000 characters (~2 000 tokens).

    This function **never raises**.  On any failure it logs a warning and
    returns an empty string.

    Args:
        brain: The memory graph backend (must satisfy :class:`BrainProtocol`).
        session_id: Current session identifier (integer).
        user_message: The user's latest message (reserved for future semantic
            search; currently unused but included in the signature for
            forward-compatibility).
        llm: LLM wrapper (reserved for future embedding-based retrieval).

    Returns:
        A markdown string ready for inclusion in a system prompt, or ``""``
        if the brain contains no relevant memories.
    """
    try:
        sections: list[str] = []

        # Layer 1 -- Core Identity
        core_facts = brain.search(
            event_types=["fact"],
            min_confidence=0.8,
            sort="confidence",
            limit=10,
        )
        sections.append(
            _format_memory_section("What I Know About You", core_facts)
        )

        # Layer 2 -- Recent Context
        adjacent_ids = _derive_adjacent_session_ids(session_id)
        recent_nodes = brain.search(
            session_ids=adjacent_ids,
            sort="recent",
            limit=20,
        )
        sections.append(
            _format_memory_section("Recent Context", recent_nodes)
        )

        # Layer 3 -- Active Decisions
        decisions = brain.search(
            event_types=["decision"],
            sort="recent",
            limit=5,
        )
        sections.append(
            _format_memory_section("Recent Decisions I've Made", decisions)
        )

        # Layer 4 -- Corrections
        corrections = brain.search(
            event_types=["correction"],
            sort="recent",
            limit=5,
        )
        sections.append(
            _format_memory_section("Corrections (Updated Information)", corrections)
        )

        # Combine non-empty sections
        body = "\n".join(s for s in sections if s)
        if not body.strip():
            return ""

        context = _CONTEXT_HEADER + "\n" + body
        return _truncate(context)

    except Exception as exc:
        logger.warning("Failed to build memory context: %s", exc)
        return ""


# ---------------------------------------------------------------------------
# Public API -- extract_and_store
# ---------------------------------------------------------------------------

def extract_and_store(
    brain: BrainProtocol,
    llm: Any,
    user_message: str,
    assistant_response: str,
    session_id: int,
) -> None:
    """Run the full extraction-and-storage pipeline for one conversation turn.

    Steps:

    1. Fetch existing recent facts from the brain for relationship grounding.
    2. Call :func:`extract_events` to obtain an :class:`ExtractionResult`.
    3. Write each extracted event to the brain via the appropriate
       ``add_*`` method.
    4. Resolve relationship links between new and existing nodes.
    5. Process corrections -- find the superseded node and write the
       correction.

    This function **never raises**.  All errors are logged and swallowed.

    Args:
        brain: The memory graph backend.
        llm: LLM wrapper for the extraction prompt.
        user_message: The raw user message text.
        assistant_response: The assistant's reply text.
        session_id: Current session identifier (integer).
    """
    try:
        # 1. Gather existing memories for grounding
        existing_memories: list[dict[str, Any]] = []
        try:
            existing_memories = brain.get_recent_facts(limit=50)
        except Exception as exc:
            logger.debug("Could not fetch existing memories: %s", exc)

        # 2. Extract events
        result: ExtractionResult = extract_events(
            llm=llm,
            user_message=user_message,
            assistant_response=assistant_response,
            existing_memories=existing_memories,
        )

        if not result.events and not result.corrections:
            logger.debug("No events or corrections extracted -- nothing to store.")
            return

        # 3. Write events to brain
        # brain.add_* methods return an int (node ID), not a dict.
        new_nodes: list[tuple[int, list[dict[str, Any]]]] = []

        for event in result.events:
            try:
                method_name = _EVENT_TYPE_TO_ADD_METHOD.get(event.event_type)
                if method_name is None:
                    logger.debug(
                        "Unknown event_type '%s' -- skipping.", event.event_type
                    )
                    continue

                add_method = getattr(brain, method_name, None)
                if add_method is None:
                    logger.debug(
                        "Brain has no method '%s' -- skipping.", method_name
                    )
                    continue

                node_id: int = add_method(
                    content=event.content,
                    session_id=session_id,
                    confidence=event.confidence,
                )
                new_nodes.append((node_id, event.relationships))
                logger.debug(
                    "Stored %s (node %d): %s (confidence=%.2f)",
                    event.event_type,
                    node_id,
                    event.content[:80],
                    event.confidence,
                )
            except Exception as exc:
                logger.warning(
                    "Failed to store event '%s': %s",
                    event.content[:60],
                    exc,
                )

        # 4. Resolve relationships
        for node_id, relationships in new_nodes:
            for rel in relationships:
                try:
                    target_desc = rel.get("target_description", "")
                    matched = find_best_match(target_desc, existing_memories)
                    if matched is None:
                        continue

                    target_id = matched.get("id")
                    if target_id is None:
                        continue

                    brain.link(
                        source_id=node_id,
                        target_id=int(target_id),
                        edge_type=rel.get("edge_type", "supports"),
                        weight=float(rel.get("weight", 0.5)),
                    )
                    logger.debug(
                        "Linked node %d -> %s (%s)",
                        node_id,
                        target_id,
                        rel.get("edge_type", "supports"),
                    )
                except Exception as exc:
                    logger.debug("Failed to link relationship: %s", exc)

        # 5. Handle corrections
        for correction in result.corrections:
            try:
                matched = find_best_match(
                    correction.old_description, existing_memories
                )
                if matched is not None:
                    supersedes_id = int(matched.get("id", 0))
                else:
                    # No matching old memory found; store as a new fact instead.
                    brain.add_fact(
                        content=correction.new_content,
                        session_id=session_id,
                        confidence=correction.confidence,
                    )
                    logger.debug(
                        "No match for correction '%s'; stored as new fact.",
                        correction.old_description[:60],
                    )
                    continue

                brain.add_correction(
                    content=correction.new_content,
                    session_id=session_id,
                    supersedes_id=supersedes_id,
                )
                logger.debug(
                    "Stored correction superseding %d: %s",
                    supersedes_id,
                    correction.new_content[:80],
                )
            except Exception as exc:
                logger.warning(
                    "Failed to store correction '%s': %s",
                    correction.new_content[:60],
                    exc,
                )

    except Exception as exc:
        logger.warning("extract_and_store pipeline failed: %s", exc)
