"""Shared test fixtures for the AgenticMemory SDK test suite."""

import pytest
import tempfile
import shutil
from pathlib import Path
from unittest.mock import MagicMock


@pytest.fixture
def tmp_dir():
    """Temporary directory for brain files. Cleaned up after test."""
    d = tempfile.mkdtemp(prefix="amem_test_")
    yield d
    shutil.rmtree(d, ignore_errors=True)


@pytest.fixture
def brain_path(tmp_dir):
    """Path for a temporary brain file."""
    return str(Path(tmp_dir) / "test.amem")


@pytest.fixture
def brain(brain_path):
    """A Brain instance with a temporary brain file.
    Requires the amem CLI to be available."""
    from agentic_memory import Brain
    b = Brain(brain_path)
    return b


@pytest.fixture
def mock_provider():
    """A mocked LLM provider for testing without API calls."""
    from agentic_memory.integrations.base import LLMProvider, ChatResponse

    provider = MagicMock(spec=LLMProvider)
    provider.chat.return_value = ChatResponse(
        content="Hello! I'm a mock response.",
        model="mock-model",
        input_tokens=10,
        output_tokens=5,
    )
    provider.chat_json.return_value = {
        "events": [
            {
                "type": "fact",
                "content": "User said hello",
                "confidence": 0.8,
                "relationships": [],
            }
        ],
        "corrections": [],
        "session_summary": "Greeting exchange",
    }
    provider.name.return_value = "MockProvider"
    return provider
