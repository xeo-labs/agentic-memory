"""Import and packaging tests for the AgenticMemory SDK."""

import pytest


def test_import_brain():
    """Brain class should be importable from the top-level package."""
    from agentic_memory import Brain
    assert Brain is not None


def test_import_models():
    """All data models should be importable from the top-level package."""
    from agentic_memory import Event, Edge, EventType, EdgeType, BrainInfo
    assert Event is not None
    assert Edge is not None
    assert EventType is not None
    assert EdgeType is not None
    assert BrainInfo is not None


def test_import_errors():
    """All exception classes should be importable from the top-level package."""
    from agentic_memory import AmemError, BrainNotFoundError, CLIError
    assert AmemError is not None
    assert BrainNotFoundError is not None
    assert CLIError is not None


def test_import_agent():
    """MemoryAgent should be importable from the top-level package."""
    from agentic_memory import MemoryAgent
    assert MemoryAgent is not None


def test_version():
    """Package version should be set."""
    from agentic_memory import __version__
    assert __version__ == "0.1.0"


def test_all_exports():
    """All names in __all__ should be accessible attributes."""
    import agentic_memory
    for name in agentic_memory.__all__:
        assert hasattr(agentic_memory, name), f"Missing export: {name}"


def test_integration_imports_without_deps():
    """Core imports should work even without LLM provider deps installed."""
    from agentic_memory import Brain
    # This must NOT raise ImportError
    assert Brain is not None


def test_integration_base_imports():
    """Base integration classes should always be importable."""
    from agentic_memory.integrations import LLMProvider, ChatMessage, ChatResponse
    assert LLMProvider is not None
    assert ChatMessage is not None
    assert ChatResponse is not None
