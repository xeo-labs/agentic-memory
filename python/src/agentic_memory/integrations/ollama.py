"""Ollama (local models) LLM provider.

Requires: ``pip install agentic-memory[ollama]``

Also requires Ollama to be running locally (``ollama serve``).
"""

from __future__ import annotations

import json
import logging
from typing import Any

from agentic_memory.errors import ProviderError
from agentic_memory.integrations.base import (
    ChatMessage,
    ChatResponse,
    LLMProvider,
    sanitize_json_text,
)

logger = logging.getLogger(__name__)


class OllamaProvider(LLMProvider):
    """Local models via Ollama.

    Requires: pip install agentic-memory[ollama]
    Also requires Ollama to be running locally.

    Args:
        model: Model name (default: "llama3.2").
        base_url: Ollama API URL (default: "http://localhost:11434").

    Example:
        >>> provider = OllamaProvider(model="llama3.2")
        >>> response = provider.chat([ChatMessage(role="user", content="Hello")])
    """

    def __init__(
        self,
        model: str = "llama3.2",
        base_url: str = "http://localhost:11434",
    ) -> None:
        try:
            import httpx
        except ImportError:
            raise ImportError(
                "Ollama provider requires httpx. "
                "Install with: pip install agentic-memory[ollama]"
            )

        self._model = model
        self._base_url = base_url.rstrip("/")
        self._httpx = httpx

    def chat(self, messages: list[ChatMessage]) -> ChatResponse:
        """Send messages and get a response from Ollama.

        Args:
            messages: List of conversation messages.

        Returns:
            ChatResponse with the model's reply.
        """
        ollama_messages = [
            {"role": m.role, "content": m.content}
            for m in messages
        ]

        try:
            response = self._httpx.post(
                f"{self._base_url}/api/chat",
                json={
                    "model": self._model,
                    "messages": ollama_messages,
                    "stream": False,
                },
                timeout=120.0,
            )
            response.raise_for_status()
        except Exception as e:
            raise ProviderError(f"Ollama API error: {e}") from e

        data = response.json()
        message = data.get("message", {})
        content = message.get("content", "")

        # Ollama token usage
        input_tokens = data.get("prompt_eval_count", 0)
        output_tokens = data.get("eval_count", 0)

        return ChatResponse(
            content=content,
            model=self._model,
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
                f"Failed to parse JSON from Ollama response: {e}\n"
                f"Raw content: {response.content[:500]}"
            ) from e

    def name(self) -> str:
        """Human-readable provider name."""
        return f"Ollama ({self._model})"
