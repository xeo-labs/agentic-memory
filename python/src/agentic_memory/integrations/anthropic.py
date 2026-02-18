"""Anthropic (Claude) LLM provider.

Requires: ``pip install agentic-memory[anthropic]``
"""

from __future__ import annotations

import json
import logging
import os
from typing import Any

from agentic_memory.errors import ProviderError
from agentic_memory.integrations.base import (
    ChatMessage,
    ChatResponse,
    LLMProvider,
    sanitize_json_text,
)

logger = logging.getLogger(__name__)


class AnthropicProvider(LLMProvider):
    """Claude via the Anthropic API.

    Requires: pip install agentic-memory[anthropic]

    Args:
        api_key: Anthropic API key. If None, reads from ANTHROPIC_API_KEY env var.
        model: Model name (default: "claude-sonnet-4-20250514").
        max_tokens: Maximum tokens in the response (default: 4096).

    Example:
        >>> provider = AnthropicProvider(api_key="sk-ant-...")
        >>> response = provider.chat([ChatMessage(role="user", content="Hello")])
    """

    def __init__(
        self,
        api_key: str | None = None,
        model: str = "claude-sonnet-4-20250514",
        max_tokens: int = 4096,
    ) -> None:
        try:
            import anthropic
        except ImportError:
            raise ImportError(
                "Anthropic provider requires the anthropic package. "
                "Install with: pip install agentic-memory[anthropic]"
            )

        self._api_key = api_key or os.environ.get("ANTHROPIC_API_KEY")
        if not self._api_key:
            raise ProviderError(
                "No Anthropic API key. Set ANTHROPIC_API_KEY or pass api_key="
            )
        self._model = model
        self._max_tokens = max_tokens
        self._client = anthropic.Anthropic(api_key=self._api_key)

    def chat(self, messages: list[ChatMessage]) -> ChatResponse:
        """Send messages and get a response from Claude.

        Args:
            messages: List of conversation messages.

        Returns:
            ChatResponse with the model's reply.
        """
        # Separate system messages from the conversation
        system_parts = [m.content for m in messages if m.role == "system"]
        system_text = "\n\n".join(system_parts) if system_parts else ""
        conv_messages = [
            {"role": m.role, "content": m.content}
            for m in messages
            if m.role != "system"
        ]

        try:
            kwargs: dict[str, Any] = {
                "model": self._model,
                "max_tokens": self._max_tokens,
                "messages": conv_messages,
            }
            if system_text:
                kwargs["system"] = system_text

            response = self._client.messages.create(**kwargs)
        except Exception as e:
            raise ProviderError(f"Anthropic API error: {e}") from e

        content = ""
        if response.content:
            content = response.content[0].text

        return ChatResponse(
            content=content,
            model=response.model,
            input_tokens=response.usage.input_tokens,
            output_tokens=response.usage.output_tokens,
        )

    def chat_json(self, messages: list[ChatMessage]) -> dict[str, Any]:
        """Send messages and get a JSON-parsed response.

        Sanitizes the output to handle markdown fences and prose
        that LLMs sometimes wrap around JSON.

        Args:
            messages: List of conversation messages.

        Returns:
            Parsed JSON dictionary.
        """
        response = self.chat(messages)
        cleaned = sanitize_json_text(response.content)
        try:
            result = json.loads(cleaned)
            if isinstance(result, dict):
                return result
            return {"result": result}
        except json.JSONDecodeError as e:
            raise ProviderError(
                f"Failed to parse JSON from Anthropic response: {e}\n"
                f"Raw content: {response.content[:500]}"
            ) from e

    def name(self) -> str:
        """Human-readable provider name."""
        return f"Anthropic ({self._model})"
