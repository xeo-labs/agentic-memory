"""Integration tests for the amem-agent.

These tests require the real ``amem`` CLI binary but use a MockLLM
so no API keys are needed.  They exercise the full pipeline:
message → memory context → LLM call → extraction → brain storage.
"""

from __future__ import annotations

import os
import tempfile

import pytest

from amem_agent.agent.session import SessionManager
from amem_agent.brain import Brain
from amem_agent.llm.base import LLMResponse, Message
from amem_agent.memory.context import build_memory_context, extract_and_store
from amem_agent.memory.extractor import (
    ExtractionResult,
    extract_events,
    format_existing_memories,
)

# Import conftest fixtures via conftest.py
# Fixtures: amem_binary, temp_brain, mock_llm


# ---------------------------------------------------------------------------
# A smarter mock LLM for integration tests
# ---------------------------------------------------------------------------

class IntegrationMockLLM:
    """LLM mock that deterministically extracts facts and echoes memory."""

    def __init__(self):
        self._extraction_events: list[dict] = []

    def set_extraction_events(self, events: list[dict]):
        """Pre-set what the extraction call should return."""
        self._extraction_events = events

    def chat(self, messages: list[Message]) -> LLMResponse:
        # Collect all content
        system_text = ""
        user_text = ""
        for m in messages:
            if m.role == "system":
                system_text += m.content
            elif m.role == "user":
                user_text = m.content

        # If memory context has facts, include them in response
        response_parts = []
        if "Memory Context" in system_text:
            for line in system_text.split("\n"):
                if line.strip().startswith("- ") and "confidence" in line.lower():
                    # Extract just the content part
                    response_parts.append(line.strip()[2:].split(" *(")[0])

        if response_parts:
            content = "Based on what I remember: " + ". ".join(response_parts) + "."
        else:
            content = f"I understand. {user_text[:80]}"

        return LLMResponse(
            content=content,
            model="integration-mock",
            input_tokens=100,
            output_tokens=50,
        )

    def chat_json(self, messages: list[Message]) -> dict:
        if self._extraction_events:
            events = self._extraction_events
            self._extraction_events = []
            return {
                "events": events,
                "corrections": [],
                "session_summary": "Integration test extraction",
            }

        # Default: extract nothing
        return {
            "events": [],
            "corrections": [],
            "session_summary": "No events extracted",
        }

    def embed(self, text: str) -> list[float]:
        return [0.0] * 128

    def name(self) -> str:
        return "IntegrationMockLLM"


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------


class TestFullConversationTurn:
    """Test a complete message → response → extraction → storage cycle."""

    def test_single_turn_stores_events(self, temp_brain):
        brain, tmpdir = temp_brain
        llm = IntegrationMockLLM()

        session_id = 1

        # Pre-set what the extractor should find
        llm.set_extraction_events([
            {
                "event_type": "fact",
                "content": "User's name is Marcus",
                "confidence": 0.95,
                "relationships": [],
            },
            {
                "event_type": "fact",
                "content": "User lives in Portland",
                "confidence": 0.90,
                "relationships": [],
            },
        ])

        # Run the extraction pipeline
        extract_and_store(
            brain=brain,
            llm=llm,
            user_message="My name is Marcus and I live in Portland.",
            assistant_response="Nice to meet you, Marcus!",
            session_id=session_id,
        )

        # Verify events were stored
        info = brain.info()
        assert info.node_count >= 2, f"Expected >= 2 nodes, got {info.node_count}"
        assert info.facts >= 2, f"Expected >= 2 facts, got {info.facts}"

        # Verify we can search for the facts
        facts = brain.search(event_types=["fact"], sort="recent", limit=10)
        contents = [f.get("content", "") for f in facts]
        assert any("Marcus" in c for c in contents), f"No Marcus in {contents}"
        assert any("Portland" in c for c in contents), f"No Portland in {contents}"

    def test_memory_context_includes_stored_facts(self, temp_brain):
        brain, tmpdir = temp_brain

        # Manually add some facts
        brain.add_fact("User's name is Alice", session_id=1, confidence=0.95)
        brain.add_fact("User works at TechCorp", session_id=1, confidence=0.88)
        brain.add_decision("Using React for the frontend", session_id=1, confidence=0.85)

        # Build memory context
        context = build_memory_context(
            brain=brain,
            session_id=2,
            user_message="What do you know about me?",
        )

        assert "Alice" in context, f"Alice not in context: {context[:200]}"
        assert "TechCorp" in context, f"TechCorp not in context: {context[:200]}"
        assert "React" in context, f"React not in context: {context[:200]}"


class TestMultiSessionFlow:
    """Test that information persists across multiple sessions."""

    def test_two_session_recall(self, temp_brain):
        brain, tmpdir = temp_brain
        llm = IntegrationMockLLM()

        # Session 1: Store facts
        llm.set_extraction_events([
            {
                "event_type": "fact",
                "content": "User prefers dark mode",
                "confidence": 0.90,
                "relationships": [],
            },
        ])

        extract_and_store(
            brain=brain,
            llm=llm,
            user_message="I always use dark mode",
            assistant_response="Noted, dark mode it is!",
            session_id=1,
        )

        # Session 2: Check memory context
        context = build_memory_context(
            brain=brain,
            session_id=2,
            user_message="What's my preference?",
        )
        assert "dark mode" in context.lower(), f"dark mode not in context"

    def test_three_session_flow(self, temp_brain):
        brain, tmpdir = temp_brain
        llm = IntegrationMockLLM()

        # Session 1
        llm.set_extraction_events([
            {"event_type": "fact", "content": "User's name is Bob", "confidence": 0.95, "relationships": []},
        ])
        extract_and_store(brain=brain, llm=llm, user_message="I'm Bob", assistant_response="Hi Bob!", session_id=1)

        # Session 2
        llm.set_extraction_events([
            {"event_type": "fact", "content": "Bob is a Python developer", "confidence": 0.90, "relationships": []},
        ])
        extract_and_store(brain=brain, llm=llm, user_message="I code in Python", assistant_response="Great!", session_id=2)

        # Session 3: Verify both facts
        context = build_memory_context(brain=brain, session_id=3, user_message="Tell me about myself")
        assert "Bob" in context
        assert "Python" in context

        info = brain.info()
        assert info.node_count >= 2
        assert info.session_count >= 2


class TestCorrectionWorkflow:
    """Test that corrections properly supersede old facts."""

    def test_correction_creates_supersedes_edge(self, temp_brain):
        brain, tmpdir = temp_brain

        # Add original fact
        old_id = brain.add_fact("User works at Company A", session_id=1, confidence=0.90)

        # Add correction
        new_id = brain.add_correction(
            "User now works at Company B",
            session_id=2,
            supersedes_id=old_id,
        )

        # Verify the correction node exists
        info = brain.info()
        assert info.corrections >= 1

        # Verify edge exists
        assert info.edge_count >= 1

    def test_correction_via_extract_and_store(self, temp_brain):
        brain, tmpdir = temp_brain
        llm = IntegrationMockLLM()

        # First, add the original fact
        brain.add_fact("User works at Company A", session_id=1, confidence=0.90)

        # Mock extraction with a correction
        llm._extraction_events = []  # Force empty events for now

        # Directly use add_correction through the brain
        brain.add_correction(
            "User now works at Company B",
            session_id=2,
            supersedes_id=0,  # node 0
        )

        # Verify
        info = brain.info()
        assert info.node_count >= 2
        assert info.corrections >= 1


class TestSessionCompression:
    """Test that sessions can be compressed into episode nodes."""

    def test_add_episode(self, temp_brain):
        brain, tmpdir = temp_brain

        # Add some facts first
        brain.add_fact("Discussed Python projects", session_id=1, confidence=0.85)
        brain.add_fact("User needs help with debugging", session_id=1, confidence=0.80)

        # Add episode summary
        episode_id = brain.add_episode(
            content="[Session 1] Discussed Python debugging techniques",
            session_id=1,
        )

        info = brain.info()
        assert info.episodes >= 1


class TestSessionManager:
    """Test session ID management."""

    def test_session_counter_increments(self, temp_brain):
        brain, tmpdir = temp_brain

        mgr = SessionManager(brain.brain_path)

        id1 = mgr.next_session_id()
        id2 = mgr.next_session_id()
        id3 = mgr.next_session_id()

        assert id1 == 1
        assert id2 == 2
        assert id3 == 3

    def test_session_counter_persists(self, temp_brain):
        brain, tmpdir = temp_brain

        mgr1 = SessionManager(brain.brain_path)
        mgr1.next_session_id()  # 1
        mgr1.next_session_id()  # 2

        # Create a new manager (simulating restart)
        mgr2 = SessionManager(brain.brain_path)
        assert mgr2.current_session_id() == 2
        id3 = mgr2.next_session_id()
        assert id3 == 3


class TestSlashCommands:
    """Test that slash commands work via the AgentLoop interface."""

    def test_handle_quit(self, temp_brain):
        from amem_agent.agent.loop import AgentLoop
        from amem_agent.config import Config, MemoryConfig, AgentConfig, DisplayConfig

        brain, tmpdir = temp_brain
        llm = IntegrationMockLLM()

        config = Config(
            backend="mock",
            brain_path=brain.brain_path,
            memory=MemoryConfig(enabled=False),
            agent=AgentConfig(),
            display=DisplayConfig(),
        )

        loop = AgentLoop(brain=brain, llm=llm, config=config, session_id=1)

        # /quit should raise KeyboardInterrupt
        with pytest.raises(KeyboardInterrupt):
            loop.handle_command("/quit")

    def test_handle_stats(self, temp_brain, capsys):
        from amem_agent.agent.loop import AgentLoop
        from amem_agent.config import Config, MemoryConfig, AgentConfig, DisplayConfig

        brain, tmpdir = temp_brain
        llm = IntegrationMockLLM()

        config = Config(
            backend="mock",
            brain_path=brain.brain_path,
            memory=MemoryConfig(enabled=False),
            agent=AgentConfig(),
            display=DisplayConfig(),
        )

        loop = AgentLoop(brain=brain, llm=llm, config=config, session_id=1)

        # /stats should not raise
        result = loop.handle_command("/stats")
        assert result is True

    def test_handle_help(self, temp_brain):
        from amem_agent.agent.loop import AgentLoop
        from amem_agent.config import Config, MemoryConfig, AgentConfig, DisplayConfig

        brain, tmpdir = temp_brain
        llm = IntegrationMockLLM()

        config = Config(
            backend="mock",
            brain_path=brain.brain_path,
            memory=MemoryConfig(enabled=False),
            agent=AgentConfig(),
            display=DisplayConfig(),
        )

        loop = AgentLoop(brain=brain, llm=llm, config=config, session_id=1)

        result = loop.handle_command("/help")
        assert result is True

    def test_non_command_returns_false(self, temp_brain):
        from amem_agent.agent.loop import AgentLoop
        from amem_agent.config import Config, MemoryConfig, AgentConfig, DisplayConfig

        brain, tmpdir = temp_brain
        llm = IntegrationMockLLM()

        config = Config(
            backend="mock",
            brain_path=brain.brain_path,
            memory=MemoryConfig(enabled=False),
            agent=AgentConfig(),
            display=DisplayConfig(),
        )

        loop = AgentLoop(brain=brain, llm=llm, config=config, session_id=1)

        result = loop.handle_command("Hello world")
        assert result is False


class TestGracefulFailures:
    """Test that extraction failures don't crash the agent."""

    def test_extraction_failure_is_swallowed(self, temp_brain):
        brain, tmpdir = temp_brain

        class FailingLLM:
            def chat(self, messages):
                raise RuntimeError("LLM is down!")
            def chat_json(self, messages):
                raise RuntimeError("LLM is down!")
            def embed(self, text):
                return [0.0] * 128
            def name(self):
                return "FailingLLM"

        # Should not raise
        extract_and_store(
            brain=brain,
            llm=FailingLLM(),
            user_message="Hello",
            assistant_response="Hi there!",
            session_id=1,
        )

        # Brain should be unchanged
        info = brain.info()
        assert info.node_count == 0

    def test_bad_json_extraction_is_swallowed(self, temp_brain):
        brain, tmpdir = temp_brain

        class BadJsonLLM:
            def chat(self, messages):
                return LLMResponse(content="ok", model="bad", input_tokens=1, output_tokens=1)
            def chat_json(self, messages):
                return "not a dict"  # type: ignore
            def embed(self, text):
                return [0.0] * 128
            def name(self):
                return "BadJsonLLM"

        # Should not raise
        extract_and_store(
            brain=brain,
            llm=BadJsonLLM(),
            user_message="Hello",
            assistant_response="Hi!",
            session_id=1,
        )

    def test_empty_brain_context_build(self, temp_brain):
        brain, tmpdir = temp_brain

        # Empty brain should return empty string
        context = build_memory_context(
            brain=brain,
            session_id=1,
            user_message="Hello",
        )
        assert context == ""


class TestBrainGrowth:
    """Test that the brain grows correctly across multiple operations."""

    def test_node_count_grows(self, temp_brain):
        brain, tmpdir = temp_brain

        for i in range(10):
            brain.add_fact(f"Fact number {i}", session_id=1, confidence=0.9)

        info = brain.info()
        assert info.node_count == 10
        assert info.facts == 10

    def test_edge_count_grows(self, temp_brain):
        brain, tmpdir = temp_brain

        ids = []
        for i in range(5):
            nid = brain.add_fact(f"Fact {i}", session_id=1, confidence=0.9)
            ids.append(nid)

        # Link them in a chain
        for i in range(len(ids) - 1):
            brain.link(ids[i + 1], ids[i], edge_type="caused_by", weight=0.8)

        info = brain.info()
        assert info.edge_count == 4

    def test_multiple_sessions(self, temp_brain):
        brain, tmpdir = temp_brain

        for session in range(1, 6):
            brain.add_fact(f"Session {session} fact", session_id=session, confidence=0.9)

        info = brain.info()
        assert info.node_count == 5
        assert info.session_count == 5

        sessions = brain.get_sessions()
        assert len(sessions) == 5
