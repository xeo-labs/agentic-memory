"""AgenticMemory — Portable binary graph memory for AI agents.

Quick start::

    >>> from agentic_memory import Brain
    >>> brain = Brain("my_agent.amem")
    >>> brain.add_fact("User prefers Rust", session=1)

With LLM integration::

    >>> from agentic_memory import Brain, MemoryAgent
    >>> from agentic_memory.integrations import AnthropicProvider
    >>> agent = MemoryAgent(Brain("agent.amem"), AnthropicProvider())
    >>> agent.chat("Hello!", session=1)
"""

from agentic_memory.brain import Brain
from agentic_memory.agent import MemoryAgent
from agentic_memory.models import (
    Event,
    Edge,
    EventType,
    EdgeType,
    BrainInfo,
    SessionInfo,
    TraversalResult,
    ImpactResult,
)
from agentic_memory.integrations.base import ChatMessage, ChatResponse
from agentic_memory.errors import (
    AmemError,
    BrainNotFoundError,
    NodeNotFoundError,
    AmemNotFoundError,
    CLIError,
    ValidationError,
    ProviderError,
)

__version__ = "0.1.0"
__all__ = [
    "Brain",
    "MemoryAgent",
    "Event",
    "Edge",
    "EventType",
    "EdgeType",
    "BrainInfo",
    "SessionInfo",
    "TraversalResult",
    "ImpactResult",
    "ChatMessage",
    "ChatResponse",
    "AmemError",
    "BrainNotFoundError",
    "NodeNotFoundError",
    "AmemNotFoundError",
    "CLIError",
    "ValidationError",
    "ProviderError",
]
