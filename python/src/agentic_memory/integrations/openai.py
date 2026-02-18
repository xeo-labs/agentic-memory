"""OpenAI (GPT) LLM provider.

Requires: ``pip install agentic-memory[openai]``
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


class OpenAIProvider(LLMProvider):
    """GPT via the OpenAI API.

    Requires: pip install agentic-memory[openai]

    Args:
        api_key: OpenAI API key. If None, reads from OPENAI_API_KEY env var.
        model: Model name (default: "gpt-4o").
        max_tokens: Maximum tokens in the response (default: 4096).

    Example:
        >>> provider = OpenAIProvider(api_key="sk-...")
        >>> response = provider.chat([ChatMessage(role="user", content="Hello")])
    """

    def __init__(
        self,
        api_key: str | None = None,
        model: str = "gpt-4o",
        max_tokens: int = 4096,
    ) -> None:
        try:
            import openai
        except ImportError:
            raise ImportError(
                "OpenAI provider requires the openai package. "
                "Install with: pip install agentic-memory[openai]"
            )

        self._api_key = api_key or os.environ.get("OPENAI_API_KEY")
        if not self._api_key:
            raise ProviderError(
                "No OpenAI API key. Set OPENAI_API_KEY or pass api_key="
            )
        self._model = model
        self._max_tokens = max_tokens
        self._client = openai.OpenAI(api_key=self._api_key)

    def chat(self, messages: list[ChatMessage]) -> ChatResponse:
        """Send messages and get a response from GPT.

        Args:
            messages: List of conversation messages.

        Returns:
            ChatResponse with the model's reply.
        """
        oai_messages = [
            {"role": m.role, "content": m.content}
            for m in messages
        ]

        try:
            response = self._client.chat.completions.create(
                model=self._model,
                messages=oai_messages,
                max_tokens=self._max_tokens,
            )
        except Exception as e:
            raise ProviderError(f"OpenAI API error: {e}") from e

        choice = response.choices[0]
        content = choice.message.content or ""

        input_tokens = 0
        output_tokens = 0
        if response.usage:
            input_tokens = response.usage.prompt_tokens
            output_tokens = response.usage.completion_tokens

        return ChatResponse(
            content=content,
            model=response.model,
            input_tokens=input_tokens,
            output_tokens=output_tokens,
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
                f"Failed to parse JSON from OpenAI response: {e}\n"
                f"Raw content: {response.content[:500]}"
            ) from e

    def name(self) -> str:
        """Human-readable provider name."""
        return f"OpenAI ({self._model})"
