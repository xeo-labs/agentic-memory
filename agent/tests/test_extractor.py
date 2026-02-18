"""Tests for the memory event extractor (amem_agent.memory.extractor).

All LLM calls are mocked -- no real API calls are made.
"""

from __future__ import annotations

from unittest.mock import MagicMock

import pytest

from amem_agent.llm.base import LLMError, Message, sanitize_json_text
from amem_agent.memory.extractor import (
    ExtractedCorrection,
    ExtractedEvent,
    ExtractionResult,
    _fallback_extract_facts,
    _parse_extraction_response,
    extract_events,
    find_best_match,
    format_existing_memories,
)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _make_llm(chat_json_return=None, chat_json_side_effect=None) -> MagicMock:
    """Create a mock LLM object with a configurable ``chat_json`` return."""
    llm = MagicMock()
    if chat_json_side_effect is not None:
        llm.chat_json.side_effect = chat_json_side_effect
    elif chat_json_return is not None:
        llm.chat_json.return_value = chat_json_return
    else:
        llm.chat_json.return_value = {
            "events": [],
            "corrections": [],
            "session_summary": "",
        }
    return llm


# ---------------------------------------------------------------------------
# extract_events
# ---------------------------------------------------------------------------


class TestExtractEventsBasic:
    def test_extract_events_basic(self):
        """Should return extracted facts from a well-formed LLM response."""
        llm = _make_llm(
            chat_json_return={
                "events": [
                    {
                        "event_type": "fact",
                        "content": "User prefers dark mode",
                        "confidence": 0.95,
                        "relationships": [],
                    },
                    {
                        "event_type": "decision",
                        "content": "We will use PostgreSQL",
                        "confidence": 0.88,
                        "relationships": [],
                    },
                ],
                "corrections": [],
                "session_summary": "Discussed UI preferences and database choice.",
            }
        )

        result = extract_events(
            llm=llm,
            user_message="I prefer dark mode. Let's use PostgreSQL.",
            assistant_response="Great choices! I'll note those.",
        )

        assert isinstance(result, ExtractionResult)
        assert len(result.events) == 2
        assert result.events[0].event_type == "fact"
        assert result.events[0].content == "User prefers dark mode"
        assert result.events[0].confidence == 0.95
        assert result.events[1].event_type == "decision"
        assert result.session_summary == "Discussed UI preferences and database choice."

        # Verify the LLM was called with a list of Message objects
        llm.chat_json.assert_called_once()
        messages = llm.chat_json.call_args[0][0]
        assert len(messages) == 2
        assert isinstance(messages[0], Message)
        assert messages[0].role == "system"
        assert isinstance(messages[1], Message)
        assert messages[1].role == "user"


class TestExtractEventsWithCorrections:
    def test_extract_events_with_corrections(self):
        """Should parse corrections alongside events."""
        llm = _make_llm(
            chat_json_return={
                "events": [
                    {
                        "event_type": "fact",
                        "content": "User's name is Bob",
                        "confidence": 0.99,
                    }
                ],
                "corrections": [
                    {
                        "old_description": "User's name is Alice",
                        "new_content": "User's name is Bob",
                        "confidence": 0.99,
                    }
                ],
                "session_summary": "Name correction.",
            }
        )

        result = extract_events(
            llm=llm,
            user_message="Actually, my name is Bob, not Alice.",
            assistant_response="Thanks for the correction, Bob!",
        )

        assert len(result.events) == 1
        assert len(result.corrections) == 1
        assert result.corrections[0].old_description == "User's name is Alice"
        assert result.corrections[0].new_content == "User's name is Bob"
        assert result.corrections[0].confidence == 0.99


class TestExtractEventsEmptyInput:
    def test_extract_events_empty_input(self):
        """Should return an empty ExtractionResult when both inputs are empty."""
        llm = _make_llm()

        result = extract_events(
            llm=llm,
            user_message="",
            assistant_response="",
        )

        assert isinstance(result, ExtractionResult)
        assert result.events == []
        assert result.corrections == []
        assert result.session_summary == ""
        # LLM should NOT have been called for empty input.
        llm.chat_json.assert_not_called()


class TestExtractEventsBadJson:
    def test_extract_events_bad_json(self):
        """Should return empty result when LLM raises an exception."""
        llm = _make_llm(chat_json_side_effect=LLMError("API unavailable"))

        result = extract_events(
            llm=llm,
            user_message="Hello",
            assistant_response="Hi there!",
        )

        assert isinstance(result, ExtractionResult)
        assert result.events == []
        assert result.corrections == []


class TestExtractEventsPartialJson:
    def test_extract_events_partial_json(self):
        """Partial JSON (missing 'events' key) should still parse safely."""
        llm = _make_llm(
            chat_json_return={
                "session_summary": "Just a summary, no events.",
                # Missing 'events' and 'corrections' keys entirely
            }
        )

        result = extract_events(
            llm=llm,
            user_message="Tell me about X",
            assistant_response="X is ...",
        )

        assert isinstance(result, ExtractionResult)
        assert result.events == []
        assert result.corrections == []
        assert result.session_summary == "Just a summary, no events."


# ---------------------------------------------------------------------------
# format_existing_memories
# ---------------------------------------------------------------------------


class TestFormatExistingMemories:
    def test_format_existing_memories(self):
        """Should produce a numbered list with ID, type, content, and confidence."""
        nodes = [
            {"id": 42, "event_type": "fact", "content": "User likes Python", "confidence": 0.95},
            {"id": 7, "event_type": "decision", "content": "Use PostgreSQL", "confidence": 0.88},
        ]

        result = format_existing_memories(nodes)

        assert "1. [ID:42] FACT: User likes Python (confidence: 95%)" in result
        assert "2. [ID:7] DECISION: Use PostgreSQL (confidence: 88%)" in result

    def test_format_existing_memories_empty(self):
        """Empty list should return the 'no existing memories' marker."""
        assert format_existing_memories([]) == "(no existing memories)"
        assert format_existing_memories(None) == "(no existing memories)"


# ---------------------------------------------------------------------------
# find_best_match
# ---------------------------------------------------------------------------


class TestFindBestMatch:
    def test_find_best_match_basic(self):
        """Should return the candidate with highest keyword overlap."""
        candidates = [
            {"id": 1, "content": "User prefers dark mode interface"},
            {"id": 2, "content": "Project uses PostgreSQL database"},
            {"id": 3, "content": "User likes Python programming language"},
        ]

        result = find_best_match("dark mode preference", candidates)

        assert result is not None
        assert result["id"] == 1

    def test_find_best_match_no_candidates(self):
        """Should return None when candidates list is empty."""
        assert find_best_match("anything", []) is None
        assert find_best_match("", [{"id": 1, "content": "stuff"}]) is None
        assert find_best_match("something", None) is None


# ---------------------------------------------------------------------------
# _parse_extraction_response
# ---------------------------------------------------------------------------


class TestParseExtractionResponse:
    def test_parse_extraction_response(self):
        """Should parse a fully-formed LLM JSON response into an ExtractionResult."""
        raw = {
            "events": [
                {
                    "event_type": "fact",
                    "content": "User works at Acme Corp",
                    "confidence": 0.92,
                    "relationships": [
                        {
                            "target_description": "User is a software engineer",
                            "edge_type": "supports",
                            "weight": 0.7,
                        }
                    ],
                },
                {
                    "event_type": "inference",
                    "content": "User likely uses macOS",
                    "confidence": 0.6,
                    "relationships": [],
                },
            ],
            "corrections": [
                {
                    "old_description": "User works at BigCo",
                    "new_content": "User works at Acme Corp",
                    "confidence": 0.95,
                }
            ],
            "session_summary": "Discussed user's workplace.",
        }

        result = _parse_extraction_response(raw)

        assert isinstance(result, ExtractionResult)
        assert len(result.events) == 2
        assert result.events[0].event_type == "fact"
        assert result.events[0].content == "User works at Acme Corp"
        assert result.events[0].confidence == 0.92
        assert len(result.events[0].relationships) == 1
        assert result.events[0].relationships[0]["edge_type"] == "supports"

        assert result.events[1].event_type == "inference"
        assert result.events[1].confidence == 0.6

        assert len(result.corrections) == 1
        assert result.corrections[0].old_description == "User works at BigCo"
        assert result.corrections[0].new_content == "User works at Acme Corp"

        assert result.session_summary == "Discussed user's workplace."

    def test_parse_extraction_response_empty_content_skipped(self):
        """Events with empty content should be silently skipped."""
        raw = {
            "events": [
                {"event_type": "fact", "content": "", "confidence": 0.9},
                {"event_type": "fact", "content": "Valid content", "confidence": 0.8},
            ],
            "corrections": [
                {"old_description": "", "new_content": "new", "confidence": 0.9},
                {"old_description": "old", "new_content": "", "confidence": 0.9},
            ],
            "session_summary": "",
        }

        result = _parse_extraction_response(raw)

        assert len(result.events) == 1
        assert result.events[0].content == "Valid content"
        # Both corrections should be dropped (one missing old_description, one missing new_content)
        assert len(result.corrections) == 0

    def test_parse_extraction_response_clamps_confidence(self):
        """Confidence values outside [0, 1] should be clamped."""
        raw = {
            "events": [
                {"event_type": "fact", "content": "Over-confident", "confidence": 1.5},
                {"event_type": "fact", "content": "Under-confident", "confidence": -0.2},
            ],
            "corrections": [],
            "session_summary": "",
        }

        result = _parse_extraction_response(raw)

        assert result.events[0].confidence == 1.0
        assert result.events[1].confidence == 0.0


# ---------------------------------------------------------------------------
# sanitize_json_text
# ---------------------------------------------------------------------------


class TestSanitizeJsonText:
    def test_clean_json_unchanged(self):
        """Valid JSON should pass through untouched."""
        raw = '{"events": [], "corrections": []}'
        assert sanitize_json_text(raw) == raw

    def test_strip_markdown_fences(self):
        """Should remove ```json ... ``` fences."""
        raw = '```json\n{"events": [], "corrections": []}\n```'
        result = sanitize_json_text(raw)
        import json
        parsed = json.loads(result)
        assert parsed == {"events": [], "corrections": []}

    def test_strip_bare_fences(self):
        """Should remove ``` ... ``` fences (no language tag)."""
        raw = '```\n{"name": "test"}\n```'
        result = sanitize_json_text(raw)
        import json
        parsed = json.loads(result)
        assert parsed == {"name": "test"}

    def test_strip_leading_prose(self):
        """Should extract JSON from prose like 'Here is the JSON: {...}'."""
        raw = 'Here is the JSON response:\n{"events": [{"event_type": "fact", "content": "hello"}]}'
        result = sanitize_json_text(raw)
        import json
        parsed = json.loads(result)
        assert "events" in parsed

    def test_fences_within_prose(self):
        """Should extract JSON from fences embedded in prose."""
        raw = 'Sure, here is the result:\n```json\n{"key": "value"}\n```\nHope that helps!'
        result = sanitize_json_text(raw)
        import json
        parsed = json.loads(result)
        assert parsed == {"key": "value"}

    def test_empty_input(self):
        """Empty input should return '{}'."""
        assert sanitize_json_text("") == "{}"
        assert sanitize_json_text(None) == "{}"

    def test_bom_stripped(self):
        """Unicode BOM should be removed."""
        raw = '\ufeff{"key": "value"}'
        result = sanitize_json_text(raw)
        import json
        parsed = json.loads(result)
        assert parsed == {"key": "value"}

    def test_array_json(self):
        """Should handle JSON arrays too."""
        raw = '```json\n[1, 2, 3]\n```'
        result = sanitize_json_text(raw)
        import json
        parsed = json.loads(result)
        assert parsed == [1, 2, 3]

    def test_nested_braces(self):
        """Should handle nested JSON objects correctly."""
        raw = 'Output:\n{"events": [{"type": "fact", "data": {"nested": true}}]}'
        result = sanitize_json_text(raw)
        import json
        parsed = json.loads(result)
        assert parsed["events"][0]["data"]["nested"] is True


# ---------------------------------------------------------------------------
# _fallback_extract_facts (regex-based extraction)
# ---------------------------------------------------------------------------


class TestFallbackExtractFacts:
    def test_extract_name(self):
        """Should extract name facts."""
        result = _fallback_extract_facts("My name is Marcus")
        assert len(result.events) >= 1
        assert any("Marcus" in e.content for e in result.events)

    def test_extract_location(self):
        """Should extract location facts."""
        result = _fallback_extract_facts("I live in Portland")
        assert len(result.events) >= 1
        assert any("Portland" in e.content for e in result.events)

    def test_extract_workplace(self):
        """Should extract workplace facts."""
        result = _fallback_extract_facts("I work at Google")
        assert len(result.events) >= 1
        assert any("Google" in e.content for e in result.events)

    def test_extract_multiple_facts(self):
        """Should extract multiple facts from one message."""
        result = _fallback_extract_facts(
            "My name is Alice and I live in Seattle"
        )
        assert len(result.events) >= 2
        names = [e.content for e in result.events]
        assert any("Alice" in c for c in names)
        assert any("Seattle" in c for c in names)

    def test_empty_message(self):
        """Should return empty result for empty input."""
        result = _fallback_extract_facts("")
        assert result.events == []

    def test_no_facts(self):
        """Should return empty result when no patterns match."""
        result = _fallback_extract_facts("Hello, how are you today?")
        assert result.events == []

    def test_decision_pattern(self):
        """Should extract decisions."""
        result = _fallback_extract_facts("We decided to use PostgreSQL")
        assert len(result.events) >= 1
        assert any(e.event_type == "decision" for e in result.events)

    def test_confidence_is_moderate(self):
        """Fallback events should have lower confidence than LLM extraction."""
        result = _fallback_extract_facts("My name is Bob")
        assert len(result.events) >= 1
        for event in result.events:
            assert event.confidence <= 0.8  # Below typical LLM confidence


class TestExtractEventsWithFallback:
    def test_llm_error_triggers_fallback(self):
        """When LLM raises LLMError, regex fallback should produce results."""
        llm = _make_llm(chat_json_side_effect=LLMError("API unavailable"))

        result = extract_events(
            llm=llm,
            user_message="My name is Marcus and I live in Portland",
            assistant_response="Nice to meet you, Marcus!",
        )

        # Fallback should have extracted facts
        assert isinstance(result, ExtractionResult)
        assert len(result.events) >= 1
        names = [e.content for e in result.events]
        assert any("Marcus" in c for c in names)

    def test_empty_structured_triggers_fallback(self):
        """When LLM returns empty events, regex fallback should fill in."""
        llm = _make_llm(chat_json_return={
            "events": [],
            "corrections": [],
            "session_summary": "",
        })

        result = extract_events(
            llm=llm,
            user_message="I work at Netflix and I live in Los Angeles",
            assistant_response="That sounds great!",
        )

        # Fallback should have found facts
        assert len(result.events) >= 1

    def test_successful_llm_no_fallback(self):
        """When LLM returns good events, fallback should NOT override."""
        llm = _make_llm(chat_json_return={
            "events": [
                {"event_type": "fact", "content": "LLM extracted this", "confidence": 0.95},
            ],
            "corrections": [],
            "session_summary": "Summary.",
        })

        result = extract_events(
            llm=llm,
            user_message="My name is Bob",
            assistant_response="Hi Bob!",
        )

        # Should use LLM result, not fallback
        assert len(result.events) == 1
        assert result.events[0].content == "LLM extracted this"
        assert result.events[0].confidence == 0.95
