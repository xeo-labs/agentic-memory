"""Shared pytest fixtures for amem-agent tests."""

from __future__ import annotations

import json
import os
import tempfile
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any
from unittest.mock import MagicMock, patch

import pytest

from amem_agent.brain import Brain, BrainInfo
from amem_agent.config import AgentConfig, Config, DisplayConfig, MemoryConfig
from amem_agent.llm.base import LLMResponse, Message, normalize_embedding


# ---------------------------------------------------------------------------
# Paths
# ---------------------------------------------------------------------------

AMEM_BINARY = os.environ.get(
    "AMEM_BINARY",
    "/Users/omoshola/Documents/agentic-revolution/agentic-memory/target/release/amem",
)


# ---------------------------------------------------------------------------
# MockLLM
# ---------------------------------------------------------------------------

class MockLLM:
    """A mock LLM backend that returns predictable responses.

    For chat(): Reads the memory context from the system prompt and echoes
    back relevant facts when asked questions.

    For chat_json(): Parses the conversation for factual statements and
    returns them as extraction events.
    """

    def __init__(self, canned_response: str | None = None):
        self._canned_response = canned_response
        self._call_count = 0

    def chat(self, messages: list[Message]) -> LLMResponse:
        self._call_count += 1

        # Find the system message to extract memory context
        system_content = ""
        user_content = ""
        for m in messages:
            if m.role == "system":
                system_content += m.content + "\n"
            elif m.role == "user":
                user_content = m.content

        if self._canned_response:
            return LLMResponse(
                content=self._canned_response,
                model="mock-model",
                input_tokens=100,
                output_tokens=50,
            )

        # Build a response that echoes back memory context facts
        response = self._build_response(system_content, user_content)
        return LLMResponse(
            content=response,
            model="mock-model",
            input_tokens=len(system_content + user_content) // 4,
            output_tokens=len(response) // 4,
        )

    def chat_json(self, messages: list[Message]) -> dict:
        self._call_count += 1

        # Find user message content
        user_content = ""
        for m in messages:
            if m.role == "user":
                user_content = m.content

        # Extract factual statements from conversation
        events = self._extract_events(user_content)
        return {
            "events": events,
            "corrections": [],
            "session_summary": f"Mock extraction from turn {self._call_count}",
        }

    def embed(self, text: str) -> list[float]:
        return normalize_embedding([], target_dim=128)

    def name(self) -> str:
        return "MockLLM (test)"

    def _build_response(self, system_content: str, user_content: str) -> str:
        """Build a response that references facts from memory context."""
        question_lower = user_content.lower()

        # Look for facts in memory context
        facts = []
        for line in system_content.split("\n"):
            line_stripped = line.strip()
            if line_stripped.startswith("- ") and "confidence" in line_stripped:
                facts.append(line_stripped[2:])

        # If asking a question, try to find relevant facts
        if "?" in user_content and facts:
            relevant = []
            for fact in facts:
                fact_lower = fact.lower()
                # Check if any question words overlap with fact
                question_words = set(question_lower.split())
                fact_words = set(fact_lower.split())
                if len(question_words & fact_words) > 1:
                    relevant.append(fact)
            if relevant:
                return "Based on what I know: " + "; ".join(relevant)

        return f"I understand. You said: {user_content[:100]}"

    def _extract_events(self, user_content: str) -> list[dict]:
        """Extract events from conversation content."""
        events = []

        # Look for "User message:" section
        if "User message" in user_content or "## User message" in user_content:
            # Try to find factual statements
            lines = user_content.split("\n")
            for line in lines:
                line = line.strip()
                if not line or line.startswith("#") or line.startswith("Analyze"):
                    continue
                # Skip the assistant response section
                if "Assistant response" in line:
                    break
                if "Existing memories" in line:
                    break
                if len(line) > 10 and not line.startswith("("):
                    events.append({
                        "event_type": "fact",
                        "content": line,
                        "confidence": 0.85,
                        "relationships": [],
                    })
                    if len(events) >= 3:
                        break

        return events


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------

@pytest.fixture
def amem_binary():
    """Return the path to the amem binary."""
    if not Path(AMEM_BINARY).is_file():
        pytest.skip(f"amem binary not found at {AMEM_BINARY}")
    return AMEM_BINARY


@pytest.fixture
def temp_brain(amem_binary):
    """Create a temporary brain file and return (brain, tmpdir)."""
    with tempfile.TemporaryDirectory() as tmpdir:
        brain_path = os.path.join(tmpdir, "test.amem")
        brain = Brain(brain_path=brain_path, amem_binary=amem_binary)
        brain.ensure_exists()
        yield brain, tmpdir


@pytest.fixture
def mock_llm():
    """Return a MockLLM instance."""
    return MockLLM()


@pytest.fixture
def default_config():
    """Return a default Config for testing."""
    return Config(
        backend="anthropic",
        model="mock-model",
        anthropic_api_key="test-key",
        brain_path="/tmp/test-brain.amem",
        verbose=False,
        memory=MemoryConfig(enabled=True),
        agent=AgentConfig(max_history=5),
        display=DisplayConfig(),
    )
