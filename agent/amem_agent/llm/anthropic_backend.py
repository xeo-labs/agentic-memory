"""
Anthropic (Claude) LLM backend.

Uses the ``anthropic`` Python SDK to call the Messages API.  Because
Anthropic does not offer an embedding endpoint, :meth:`embed` returns a
zero vector and logs a warning.
"""

from __future__ import annotations

import json
import logging
from typing import Any

from .base import LLMBackend, LLMError, LLMResponse, Message, normalize_embedding, sanitize_json_text

logger = logging.getLogger(__name__)

_MAX_TOKENS = 4096


class AnthropicBackend(LLMBackend):
    """LLM backend powered by the Anthropic Messages API.

    Args:
        api_key: Anthropic API key.  Must not be empty.
        model: Model identifier to use for chat completions.
            Defaults to ``"claude-sonnet-4-20250514"``.

    Raises:
        LLMError: If *api_key* is missing or the ``anthropic`` package is
            not installed.
    """

    def __init__(
        self,
        api_key: str,
        model: str = "claude-sonnet-4-20250514",
    ) -> None:
        if not api_key:
            raise LLMError(
                "Anthropic API key is required. "
                "Set the ANTHROPIC_API_KEY environment variable or pass it explicitly."
            )

        try:
            import anthropic  # noqa: F811
        except ImportError as exc:
            raise LLMError(
                "The 'anthropic' package is required for AnthropicBackend. "
                "Install it with: pip install anthropic"
            ) from exc

        self.model = model
        self._client = anthropic.Anthropic(api_key=api_key)
        logger.info("AnthropicBackend initialised with model=%s", self.model)

    # ------------------------------------------------------------------
    # chat
    # ------------------------------------------------------------------

    def chat(self, messages: list[Message]) -> LLMResponse:
        """Send *messages* to the Anthropic Messages API and return the result.

        If the conversation contains a message with ``role="system"`` it is
        extracted and passed via the dedicated ``system`` parameter, because
        the Anthropic API does not accept ``"system"`` as a message role.

        Args:
            messages: Ordered conversation history.

        Returns:
            An :class:`LLMResponse` populated from the API response.

        Raises:
            LLMError: On any API, network, or rate-limit error.
        """
        system_text, api_messages = self._split_system(messages)

        try:
            kwargs: dict[str, Any] = dict(
                model=self.model,
                max_tokens=_MAX_TOKENS,
                messages=api_messages,
            )
            if system_text:
                kwargs["system"] = system_text

            response = self._client.messages.create(**kwargs)
        except Exception as exc:
            raise LLMError(f"Anthropic chat request failed: {exc}") from exc

        content = response.content[0].text if response.content else ""
        return LLMResponse(
            content=content,
            model=response.model,
            input_tokens=response.usage.input_tokens,
            output_tokens=response.usage.output_tokens,
        )

    # ------------------------------------------------------------------
    # chat_json
    # ------------------------------------------------------------------

    def chat_json(self, messages: list[Message]) -> dict:
        """Request a JSON response from the model, with one automatic retry.

        An instruction to respond exclusively in JSON is appended to the
        conversation.  If the first attempt fails to parse, a stricter
        follow-up prompt is sent.

        Args:
            messages: Ordered conversation history.

        Returns:
            Parsed JSON as a ``dict``.

        Raises:
            LLMError: If the model output cannot be parsed as JSON after the
                retry.
        """
        json_instruction = Message(
            role="user",
            content=(
                "Important: respond with valid JSON only. "
                "Do not include any text, markdown formatting, or code fences "
                "outside of the JSON object."
            ),
        )
        augmented = list(messages) + [json_instruction]

        response = self.chat(augmented)

        # Attempt 1: raw parse
        try:
            return json.loads(response.content)
        except json.JSONDecodeError:
            pass

        # Attempt 2: sanitise (strip fences, prose, BOM)
        sanitised = sanitize_json_text(response.content)
        try:
            return json.loads(sanitised)
        except json.JSONDecodeError:
            logger.warning(
                "JSON parse failed after sanitisation for model=%s; retrying with stricter prompt.",
                self.model,
            )

        # Attempt 3: retry with a stricter prompt
        retry_messages = augmented + [
            Message(role="assistant", content=response.content),
            Message(
                role="user",
                content=(
                    "Your previous response was not valid JSON. "
                    "Reply with ONLY a single JSON object starting with { and ending with }. "
                    "No explanation, no markdown, no code fences, no extra text."
                ),
            ),
        ]
        response = self.chat(retry_messages)

        # Try raw, then sanitised
        try:
            return json.loads(response.content)
        except json.JSONDecodeError:
            pass

        sanitised = sanitize_json_text(response.content)
        try:
            return json.loads(sanitised)
        except json.JSONDecodeError as exc:
            raise LLMError(
                f"Anthropic model did not return valid JSON after retry: {response.content!r}"
            ) from exc

    # ------------------------------------------------------------------
    # embed
    # ------------------------------------------------------------------

    def embed(self, text: str) -> list[float]:
        """Return a zero vector (Anthropic does not provide an embedding API).

        A warning is logged on every call to make the limitation visible.

        Args:
            text: Input text (ignored).

        Returns:
            A list of 128 zeros.
        """
        logger.warning(
            "Anthropic does not offer an embedding API. "
            "Returning a zero vector. Consider using the OpenAI or Ollama "
            "backend for embedding support."
        )
        return normalize_embedding([], target_dim=128)

    # ------------------------------------------------------------------
    # name
    # ------------------------------------------------------------------

    def name(self) -> str:
        """Return a human-readable backend identifier."""
        return f"Anthropic ({self.model})"

    # ------------------------------------------------------------------
    # Helpers
    # ------------------------------------------------------------------

    @staticmethod
    def _split_system(
        messages: list[Message],
    ) -> tuple[str | None, list[dict[str, str]]]:
        """Separate system messages from the rest of the conversation.

        The Anthropic API requires system content to be passed via a
        dedicated ``system`` parameter rather than as a message with
        ``role="system"``.

        Returns:
            A tuple of ``(system_text, api_messages)`` where *system_text*
            is ``None`` when no system message is present and
            *api_messages* is the list formatted for the API.
        """
        system_parts: list[str] = []
        api_messages: list[dict[str, str]] = []

        for msg in messages:
            if msg.role == "system":
                system_parts.append(msg.content)
            else:
                api_messages.append({"role": msg.role, "content": msg.content})

        system_text = "\n\n".join(system_parts) if system_parts else None
        return system_text, api_messages
