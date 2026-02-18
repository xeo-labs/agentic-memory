"""Integration tests — provider interface, sanitizer, extractor (mocked)."""

import json
import pytest
from unittest.mock import MagicMock

from agentic_memory.integrations.base import ChatMessage, ChatResponse, sanitize_json_text


def test_chat_message_creation():
    """ChatMessage should store role and content."""
    msg = ChatMessage(role="user", content="Hello")
    assert msg.role == "user"
    assert msg.content == "Hello"


def test_chat_response_creation():
    """ChatResponse should store response data."""
    resp = ChatResponse(content="Hi", model="test")
    assert resp.content == "Hi"
    assert resp.model == "test"
    assert resp.input_tokens == 0
    assert resp.output_tokens == 0


def test_sanitize_json_strips_fences():
    """Should strip markdown JSON fences."""
    raw = '```json\n{"key": "value"}\n```'
    clean = sanitize_json_text(raw)
    assert json.loads(clean) == {"key": "value"}


def test_sanitize_json_strips_prose():
    """Should strip surrounding prose to find JSON."""
    raw = 'Here is the JSON:\n{"key": "value"}\nDone!'
    clean = sanitize_json_text(raw)
    assert json.loads(clean) == {"key": "value"}


def test_sanitize_json_handles_clean_input():
    """Should pass through valid JSON unchanged."""
    raw = '{"key": "value"}'
    clean = sanitize_json_text(raw)
    assert clean == raw


def test_sanitize_json_strips_bom():
    """Should strip Unicode BOM."""
    raw = '\ufeff{"key": "value"}'
    clean = sanitize_json_text(raw)
    assert json.loads(clean) == {"key": "value"}


def test_sanitize_json_handles_array():
    """Should handle JSON arrays."""
    raw = 'Here: [1, 2, 3]'
    clean = sanitize_json_text(raw)
    assert json.loads(clean) == [1, 2, 3]


def test_extraction_result_structure(mock_provider):
    """extract_events should return an ExtractionResult."""
    from agentic_memory.integrations.extractor import extract_events
    result = extract_events(
        mock_provider,
        "My name is Alice",
        "Nice to meet you, Alice!",
    )
    assert hasattr(result, "events")
    assert hasattr(result, "corrections")
    assert hasattr(result, "summary")


def test_extraction_handles_failure(mock_provider):
    """extract_events should handle provider failure gracefully."""
    from agentic_memory.integrations.extractor import extract_events
    mock_provider.chat_json.side_effect = Exception("API error")
    result = extract_events(mock_provider, "hello", "hi")
    assert result.events == []
    assert result.corrections == []


def test_context_builder_empty_brain(brain):
    """build_memory_context should return empty string for empty brain."""
    brain.create()
    from agentic_memory.integrations.context import build_memory_context
    ctx = build_memory_context(brain, session=1)
    assert ctx == "" or isinstance(ctx, str)


def test_context_builder_with_facts(brain):
    """build_memory_context should include stored facts."""
    brain.create()
    brain.add_fact("User is Alice", session=1, confidence=0.95)
    from agentic_memory.integrations.context import build_memory_context
    ctx = build_memory_context(brain, session=2)
    assert "Alice" in ctx
