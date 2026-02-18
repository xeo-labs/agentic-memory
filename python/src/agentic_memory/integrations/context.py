"""Memory context builder for LLM prompts.

Ported and refined from Phase 7A. Queries the brain for relevant
memories and formats them into a string for LLM system prompts.
"""

from __future__ import annotations

import logging
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from agentic_memory.brain import Brain

logger = logging.getLogger(__name__)


def build_memory_context(
    brain: Brain,
    session: int,
    user_message: str = "",
    max_tokens: int = 2000,
) -> str:
    """Build a memory context string for injection into an LLM prompt.

    Queries the brain for relevant memories and formats them into a
    string that can be included in the system prompt.

    Layers:
    1. Core identity facts (high confidence)
    2. Recent context (last 2-3 sessions)
    3. Active decisions
    4. Recent corrections

    Args:
        brain: The Brain instance to query.
        session: Current session ID.
        user_message: The user's message (for relevance filtering).
        max_tokens: Maximum approximate tokens for the context.

    Returns:
        Formatted memory context string, or empty string if brain is empty.
    """
    if not brain.exists:
        return ""

    try:
        info = brain.info()
    except Exception:
        return ""

    if info.is_empty:
        return ""

    sections: list[str] = []
    char_budget = max_tokens * 4  # Rough chars-to-tokens ratio

    # Layer 1: Core identity facts (high confidence)
    try:
        core_facts = brain.facts(limit=10, min_confidence=0.8)
        if core_facts:
            lines = ["## Core Facts"]
            for f in core_facts:
                lines.append(f"- {f.content} (confidence: {f.confidence:.1f})")
            sections.append("\n".join(lines))
    except Exception as e:
        logger.debug("Failed to load core facts: %s", e)

    # Layer 2: Recent context (sessions near current)
    try:
        recent_sessions = [session - 1, session - 2] if session > 1 else []
        recent_sessions = [s for s in recent_sessions if s > 0]
        if recent_sessions:
            recent_events = brain.search(
                sessions=recent_sessions,
                limit=10,
                sort="recent",
            )
            if recent_events:
                lines = ["## Recent Context"]
                for e in recent_events:
                    lines.append(f"- [{e.type.value}] {e.content}")
                sections.append("\n".join(lines))
    except Exception as e:
        logger.debug("Failed to load recent context: %s", e)

    # Layer 3: Active decisions
    try:
        recent_decisions = brain.decisions(limit=5)
        if recent_decisions:
            lines = ["## Active Decisions"]
            for d in recent_decisions:
                lines.append(f"- {d.content}")
            sections.append("\n".join(lines))
    except Exception as e:
        logger.debug("Failed to load decisions: %s", e)

    # Layer 4: Recent corrections
    try:
        recent_corrections = brain.corrections(limit=5)
        if recent_corrections:
            lines = ["## Corrections"]
            for c in recent_corrections:
                lines.append(f"- {c.content}")
            sections.append("\n".join(lines))
    except Exception as e:
        logger.debug("Failed to load corrections: %s", e)

    if not sections:
        return ""

    # Assemble and truncate to budget
    context = "\n\n".join(sections)
    if len(context) > char_budget:
        context = context[:char_budget] + "\n..."

    return context
