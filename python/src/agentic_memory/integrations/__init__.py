"""Optional LLM provider integrations for AgenticMemory.

These require extra dependencies. Install with::

    pip install agentic-memory[anthropic]
    pip install agentic-memory[openai]
    pip install agentic-memory[ollama]
    pip install agentic-memory[all]
"""

from agentic_memory.integrations.base import (
    LLMProvider,
    ChatMessage,
    ChatResponse,
    sanitize_json_text,
)

__all__ = ["LLMProvider", "ChatMessage", "ChatResponse", "sanitize_json_text"]

# Lazy imports for optional providers — don't fail if deps not installed.
# Each provider class already has a try/import guard in its __init__,
# so instantiation gives a clear error. The None fallback here just means
# the name is importable without crashing.

try:
    from agentic_memory.integrations.anthropic import AnthropicProvider
except ImportError:
    AnthropicProvider = None  # type: ignore[assignment,misc]

try:
    from agentic_memory.integrations.openai import OpenAIProvider
except ImportError:
    OpenAIProvider = None  # type: ignore[assignment,misc]

try:
    from agentic_memory.integrations.ollama import OllamaProvider
except ImportError:
    OllamaProvider = None  # type: ignore[assignment,misc]
