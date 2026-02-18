"""MemoryAgent tests — mocked LLM provider."""

import pytest
from unittest.mock import MagicMock

from agentic_memory import MemoryAgent
from agentic_memory.integrations.base import ChatMessage, ChatResponse


def test_agent_creation(brain, mock_provider):
    """MemoryAgent should be creatable with brain and provider."""
    brain.create()
    agent = MemoryAgent(brain=brain, provider=mock_provider)
    assert agent is not None


def test_agent_chat_returns_response(brain, mock_provider):
    """agent.chat should return a ChatResponse."""
    brain.create()
    agent = MemoryAgent(brain=brain, provider=mock_provider)
    response = agent.chat("Hello", session=1)
    assert isinstance(response, ChatResponse)
    assert len(response.content) > 0


def test_agent_chat_stores_events(brain, mock_provider):
    """agent.chat should extract and store events."""
    brain.create()
    agent = MemoryAgent(brain=brain, provider=mock_provider)
    agent.chat("My name is Alice", session=1)
    info = brain.info()
    assert info.node_count > 0  # Events were extracted and stored


def test_agent_chat_builds_context(brain, mock_provider):
    """agent.chat should include memory context in system prompt."""
    brain.create()
    brain.add_fact("User is Bob", session=1, confidence=0.95)
    agent = MemoryAgent(brain=brain, provider=mock_provider)
    agent.chat("What's my name?", session=2)
    # Verify the mock was called with messages containing memory context
    call_args = mock_provider.chat.call_args
    messages = call_args[0][0]  # First positional arg
    system_msgs = [m for m in messages if m.role == "system"]
    assert len(system_msgs) > 0
    assert "Bob" in system_msgs[0].content


def test_agent_last_extraction(brain, mock_provider):
    """last_extraction should be None initially, then set after chat."""
    brain.create()
    agent = MemoryAgent(brain=brain, provider=mock_provider)
    assert agent.last_extraction is None
    agent.chat("Hello", session=1)
    assert agent.last_extraction is not None


def test_agent_extraction_failure_doesnt_crash(brain, mock_provider):
    """Chat should still work even if extraction fails."""
    brain.create()
    mock_provider.chat_json.side_effect = Exception("JSON parse failed")
    agent = MemoryAgent(brain=brain, provider=mock_provider)
    response = agent.chat("Hello", session=1)
    assert response.content is not None


def test_agent_with_history(brain, mock_provider):
    """agent.chat should include history in messages."""
    brain.create()
    agent = MemoryAgent(brain=brain, provider=mock_provider)
    history = [
        ChatMessage(role="user", content="Previous question"),
        ChatMessage(role="assistant", content="Previous answer"),
    ]
    response = agent.chat("Follow up", session=1, history=history)
    call_args = mock_provider.chat.call_args
    messages = call_args[0][0]
    assert len(messages) >= 4  # system + 2 history + current


def test_agent_no_extraction_mode(brain, mock_provider):
    """Agent with extract_events=False should not store events."""
    brain.create()
    agent = MemoryAgent(brain=brain, provider=mock_provider, extract_events=False)
    agent.chat("Hello", session=1)
    info = brain.info()
    assert info.node_count == 0  # No extraction
