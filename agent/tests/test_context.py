"""Tests for the memory context builder (amem_agent.memory.context).

Brain and LLM objects are mocked throughout.
"""

from __future__ import annotations

from typing import Any
from unittest.mock import MagicMock, call, patch

import pytest

from amem_agent.memory.context import (
    _derive_adjacent_session_ids,
    _format_memory_section,
    _truncate,
    build_memory_context,
    extract_and_store,
)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _make_brain(
    search_returns: list[dict[str, Any]] | None = None,
    recent_facts: list[dict[str, Any]] | None = None,
) -> MagicMock:
    """Create a mock brain that returns predictable search results.

    If *search_returns* is a list, every call to ``brain.search()`` returns
    that same list.  Otherwise it returns ``[]``.
    """
    brain = MagicMock()
    brain.search.return_value = search_returns if search_returns is not None else []
    brain.get_recent_facts.return_value = recent_facts if recent_facts is not None else []
    brain.add_fact.return_value = 100
    brain.add_decision.return_value = 101
    brain.add_inference.return_value = 102
    brain.add_skill.return_value = 103
    brain.add_correction.return_value = 104
    brain.link.return_value = None
    return brain


def _make_llm(chat_json_return: dict | None = None) -> MagicMock:
    llm = MagicMock()
    llm.chat_json.return_value = chat_json_return or {
        "events": [],
        "corrections": [],
        "session_summary": "",
    }
    return llm


# ---------------------------------------------------------------------------
# build_memory_context
# ---------------------------------------------------------------------------


class TestBuildMemoryContextBasic:
    def test_build_memory_context_basic(self):
        """Should return a markdown string containing memory sections."""
        nodes = [
            {"id": 1, "event_type": "fact", "content": "User likes Rust", "confidence": 0.9},
            {"id": 2, "event_type": "fact", "content": "User works remotely", "confidence": 0.85},
        ]
        brain = _make_brain(search_returns=nodes)

        context = build_memory_context(
            brain=brain, session_id=5, user_message="Hello"
        )

        assert isinstance(context, str)
        assert "Memory Context" in context
        assert "User likes Rust" in context
        assert "User works remotely" in context
        # brain.search should have been called multiple times (core, recent, decisions, corrections)
        assert brain.search.call_count == 4


class TestBuildMemoryContextEmptyBrain:
    def test_build_memory_context_empty_brain(self):
        """Should return an empty string when the brain has no memories."""
        brain = _make_brain(search_returns=[])

        context = build_memory_context(
            brain=brain, session_id=1, user_message="Hi"
        )

        assert context == ""


class TestBuildMemoryContextTruncation:
    def test_build_memory_context_truncation(self):
        """Should truncate context that exceeds the character budget."""
        # Create a large number of nodes to blow past the limit
        nodes = [
            {
                "id": i,
                "event_type": "fact",
                "content": f"Memory item number {i} with extra text " * 10,
                "confidence": 0.9,
            }
            for i in range(200)
        ]
        brain = _make_brain(search_returns=nodes)

        context = build_memory_context(
            brain=brain, session_id=5, user_message="Hello"
        )

        # The _truncate function caps output at 8000 chars and appends a marker
        assert len(context) <= 8100  # allowing some slack for the truncation marker
        if len(context) > 7900:
            assert "memory context truncated" in context


# ---------------------------------------------------------------------------
# _derive_adjacent_session_ids
# ---------------------------------------------------------------------------


class TestDeriveAdjacentSessionIds:
    def test_session_5(self):
        """Session 5 should produce [5, 4, 3]."""
        result = _derive_adjacent_session_ids(5)
        assert result == [5, 4, 3]

    def test_session_1(self):
        """Session 1 should produce [1, 0] -- no negative IDs."""
        result = _derive_adjacent_session_ids(1)
        assert result == [1, 0]

    def test_session_0(self):
        """Session 0 should produce [0] -- all clamp to 0 and deduplicate."""
        result = _derive_adjacent_session_ids(0)
        assert result == [0]


# ---------------------------------------------------------------------------
# extract_and_store
# ---------------------------------------------------------------------------


class TestExtractAndStoreFullPipeline:
    def test_extract_and_store_full_pipeline(self):
        """Full pipeline: extract 2 events, store them, resolve relationships."""
        existing_memories = [
            {"id": 10, "event_type": "fact", "content": "User knows Python programming", "confidence": 0.9},
        ]
        brain = _make_brain(recent_facts=existing_memories)
        llm = _make_llm(
            chat_json_return={
                "events": [
                    {
                        "event_type": "fact",
                        "content": "User prefers VS Code editor",
                        "confidence": 0.88,
                        "relationships": [
                            {
                                "target_description": "Python programming",
                                "edge_type": "related_to",
                                "weight": 0.6,
                            }
                        ],
                    },
                    {
                        "event_type": "decision",
                        "content": "Will use Docker for deployment",
                        "confidence": 0.92,
                        "relationships": [],
                    },
                ],
                "corrections": [],
                "session_summary": "Discussed tools.",
            }
        )

        extract_and_store(
            brain=brain,
            llm=llm,
            user_message="I prefer VS Code. Let's use Docker.",
            assistant_response="Noted!",
            session_id=3,
        )

        # Fact should have been stored
        brain.add_fact.assert_called_once_with(
            content="User prefers VS Code editor",
            session_id=3,
            confidence=0.88,
        )
        # Decision should have been stored
        brain.add_decision.assert_called_once_with(
            content="Will use Docker for deployment",
            session_id=3,
            confidence=0.92,
        )
        # A relationship link should have been created (Python programming -> existing node 10)
        brain.link.assert_called_once()
        link_kwargs = brain.link.call_args[1]
        assert link_kwargs["target_id"] == 10
        assert link_kwargs["edge_type"] == "related_to"


class TestExtractAndStoreNoEvents:
    def test_extract_and_store_no_events(self):
        """When the LLM extracts nothing, no brain writes should occur."""
        brain = _make_brain()
        llm = _make_llm(
            chat_json_return={
                "events": [],
                "corrections": [],
                "session_summary": "Small talk.",
            }
        )

        extract_and_store(
            brain=brain,
            llm=llm,
            user_message="How's the weather?",
            assistant_response="It's sunny!",
            session_id=1,
        )

        brain.add_fact.assert_not_called()
        brain.add_decision.assert_not_called()
        brain.add_correction.assert_not_called()
        brain.link.assert_not_called()


class TestExtractAndStoreWithCorrection:
    def test_extract_and_store_with_correction(self):
        """Corrections should find the matching old memory and call add_correction."""
        existing_memories = [
            {"id": 5, "event_type": "fact", "content": "User's favorite color is blue", "confidence": 0.9},
            {"id": 6, "event_type": "fact", "content": "User works at BigCo", "confidence": 0.85},
        ]
        brain = _make_brain(recent_facts=existing_memories)
        llm = _make_llm(
            chat_json_return={
                "events": [],
                "corrections": [
                    {
                        "old_description": "favorite color blue",
                        "new_content": "User's favorite color is green",
                        "confidence": 0.95,
                    }
                ],
                "session_summary": "Color correction.",
            }
        )

        extract_and_store(
            brain=brain,
            llm=llm,
            user_message="Actually my favorite color is green.",
            assistant_response="Updated!",
            session_id=4,
        )

        brain.add_correction.assert_called_once_with(
            content="User's favorite color is green",
            session_id=4,
            supersedes_id=5,
        )


# ---------------------------------------------------------------------------
# _format_memory_section
# ---------------------------------------------------------------------------


class TestFormatMemorySection:
    def test_format_memory_section(self):
        """Should produce a markdown heading + bullet list."""
        nodes = [
            {"event_type": "fact", "content": "Knows Python", "confidence": 0.9},
            {"event_type": "decision", "content": "Use Flask", "confidence": 0.8},
        ]

        result = _format_memory_section("Test Section", nodes)

        assert "## Test Section" in result
        assert "[FACT] Knows Python" in result
        assert "90%" in result
        assert "[DECISION] Use Flask" in result
        assert "80%" in result

    def test_format_memory_section_empty(self):
        """Empty nodes should produce an empty string."""
        assert _format_memory_section("Empty", []) == ""
        assert _format_memory_section("Empty", None) == ""
