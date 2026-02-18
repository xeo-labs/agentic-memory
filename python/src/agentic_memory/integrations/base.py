"""Abstract LLM provider interface and shared utilities.

This module defines the base classes for LLM integration:

- ``LLMProvider`` — abstract interface all providers implement
- ``ChatMessage`` / ``ChatResponse`` — message data types
- ``sanitize_json_text()`` — JSON cleaning for LLM output
"""

from __future__ import annotations

import json
import re
from abc import ABC, abstractmethod
from dataclasses import dataclass
from typing import Any


@dataclass
class ChatMessage:
    """A single message in a conversation.

    Attributes:
        role: Message role — "system", "user", or "assistant".
        content: The text content of the message.
    """
    role: str
    content: str


@dataclass
class ChatResponse:
    """Response from an LLM.

    Attributes:
        content: The response text.
        model: Model identifier that generated the response.
        input_tokens: Number of input tokens consumed.
        output_tokens: Number of output tokens generated.
    """
    content: str
    model: str
    input_tokens: int = 0
    output_tokens: int = 0


class LLMProvider(ABC):
    """Abstract interface for LLM providers.

    Implement this to integrate AgenticMemory with any LLM.

    Example:
        >>> class MyProvider(LLMProvider):
        ...     def chat(self, messages):
        ...         return ChatResponse(content="Hello!", model="my-model")
        ...     def chat_json(self, messages):
        ...         return {"result": "Hello!"}
        ...     def name(self):
        ...         return "MyProvider"
    """

    @abstractmethod
    def chat(self, messages: list[ChatMessage]) -> ChatResponse:
        """Send messages and get a response.

        Args:
            messages: List of conversation messages.

        Returns:
            The LLM's response.
        """
        ...

    @abstractmethod
    def chat_json(self, messages: list[ChatMessage]) -> dict[str, Any]:
        """Send messages and get a JSON-parsed response.

        Used for structured extraction (cognitive events).
        Must handle JSON parsing failures with retries.

        Args:
            messages: List of conversation messages.

        Returns:
            Parsed JSON dictionary.
        """
        ...

    @abstractmethod
    def name(self) -> str:
        """Human-readable provider name."""
        ...


# ===================================================================
# JSON Sanitizer — ported from Phase 7A
# ===================================================================

# Patterns for stripping markdown fences
_FENCE_RE = re.compile(r"^```(?:json)?\s*\n?", re.MULTILINE)
_FENCE_END_RE = re.compile(r"\n?```\s*$", re.MULTILINE)

# Pattern to find the outermost JSON object or array
_JSON_OBJECT_RE = re.compile(r"\{[\s\S]*\}", re.DOTALL)
_JSON_ARRAY_RE = re.compile(r"\[[\s\S]*\]", re.DOTALL)


def sanitize_json_text(raw: str) -> str:
    """Strip markdown fences, prose, and artifacts from LLM JSON output.

    Handles common LLM output issues:

    - Markdown code fences (``\\`\\`\\`json ... \\`\\`\\```)
    - Leading/trailing prose around JSON
    - Unicode BOM
    - Zero-width characters

    Args:
        raw: Raw LLM output that should contain JSON.

    Returns:
        Cleaned string ready for ``json.loads()``.
    """
    # Strip Unicode BOM and zero-width characters
    text = raw.strip()
    text = text.lstrip("\ufeff")
    text = text.replace("\u200b", "").replace("\u200c", "").replace("\u200d", "")

    # Strip markdown code fences
    text = _FENCE_RE.sub("", text)
    text = _FENCE_END_RE.sub("", text)
    text = text.strip()

    # If it's already valid JSON, return it
    try:
        json.loads(text)
        return text
    except json.JSONDecodeError:
        pass

    # Try to extract the outermost JSON object
    match = _JSON_OBJECT_RE.search(text)
    if match:
        candidate = match.group(0)
        try:
            json.loads(candidate)
            return candidate
        except json.JSONDecodeError:
            pass

    # Try to extract the outermost JSON array
    match = _JSON_ARRAY_RE.search(text)
    if match:
        candidate = match.group(0)
        try:
            json.loads(candidate)
            return candidate
        except json.JSONDecodeError:
            pass

    # Return the stripped text as-is (caller will handle parse errors)
    return text
