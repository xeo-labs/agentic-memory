"""
OpenAI LLM backend.

Uses the ``openai`` Python SDK to call the Chat Completions and
Embeddings APIs.
"""

from __future__ import annotations

import json
import logging

from .base import LLMBackend, LLMError, LLMResponse, Message, normalize_embedding, sanitize_json_text

logger = logging.getLogger(__name__)

_MAX_TOKENS = 4096


class OpenAIBackend(LLMBackend):
    """LLM backend powered by the OpenAI API.

    Args:
        api_key: OpenAI API key.  Must not be empty.
        model: Model identifier for chat completions.
            Defaults to ``"gpt-4o"``.
        embedding_model: Model identifier for text embeddings.
            Defaults to ``"text-embedding-3-small"``.

    Raises:
        LLMError: If *api_key* is missing or the ``openai`` package is not
            installed.
    """

    def __init__(
        self,
        api_key: str,
        model: str = "gpt-4o",
        embedding_model: str = "text-embedding-3-small",
    ) -> None:
        if not api_key:
            raise LLMError(
                "OpenAI API key is required. "
                "Set the OPENAI_API_KEY environment variable or pass it explicitly."
            )

        try:
            import openai  # noqa: F811
        except ImportError as exc:
            raise LLMError(
                "The 'openai' package is required for OpenAIBackend. "
                "Install it with: pip install openai"
            ) from exc

        self.model = model
        self.embedding_model = embedding_model
        self._client = openai.OpenAI(api_key=api_key)
        logger.info(
            "OpenAIBackend initialised with model=%s, embedding_model=%s",
            self.model,
            self.embedding_model,
        )

    # ------------------------------------------------------------------
    # chat
    # ------------------------------------------------------------------

    def chat(self, messages: list[Message]) -> LLMResponse:
        """Send *messages* to the OpenAI Chat Completions API.

        Args:
            messages: Ordered conversation history.

        Returns:
            An :class:`LLMResponse` populated from the API response.

        Raises:
            LLMError: On any API, network, rate-limit, or timeout error.
        """
        api_messages = [{"role": m.role, "content": m.content} for m in messages]

        try:
            response = self._client.chat.completions.create(
                model=self.model,
                messages=api_messages,
                max_tokens=_MAX_TOKENS,
            )
        except Exception as exc:
            raise LLMError(f"OpenAI chat request failed: {exc}") from exc

        choice = response.choices[0]
        usage = response.usage
        return LLMResponse(
            content=choice.message.content or "",
            model=response.model,
            input_tokens=usage.prompt_tokens if usage else 0,
            output_tokens=usage.completion_tokens if usage else 0,
        )

    # ------------------------------------------------------------------
    # chat_json
    # ------------------------------------------------------------------

    def chat_json(self, messages: list[Message]) -> dict:
        """Request a JSON response using the ``response_format`` parameter.

        The OpenAI API natively supports ``{"type": "json_object"}`` which
        guarantees that the model output is syntactically valid JSON.

        Args:
            messages: Ordered conversation history.  At least one message
                should mention "JSON" so the API does not reject the request.

        Returns:
            Parsed JSON as a ``dict``.

        Raises:
            LLMError: If the API call fails or the output is not valid JSON.
        """
        api_messages = [{"role": m.role, "content": m.content} for m in messages]

        # Ensure at least one message references JSON (OpenAI requirement
        # when using response_format=json_object).
        has_json_mention = any("json" in m.content.lower() for m in messages)
        if not has_json_mention:
            api_messages.append(
                {
                    "role": "user",
                    "content": "Respond with valid JSON only.",
                }
            )

        try:
            response = self._client.chat.completions.create(
                model=self.model,
                messages=api_messages,
                max_tokens=_MAX_TOKENS,
                response_format={"type": "json_object"},
            )
        except Exception as exc:
            raise LLMError(f"OpenAI chat_json request failed: {exc}") from exc

        raw = response.choices[0].message.content or ""
        try:
            return json.loads(raw)
        except json.JSONDecodeError:
            pass

        # Safety net: sanitise and retry (shouldn't be needed with response_format)
        sanitised = sanitize_json_text(raw)
        try:
            return json.loads(sanitised)
        except json.JSONDecodeError as exc:
            raise LLMError(
                f"OpenAI returned invalid JSON despite response_format: {raw!r}"
            ) from exc

    # ------------------------------------------------------------------
    # embed
    # ------------------------------------------------------------------

    def embed(self, text: str) -> list[float]:
        """Create a text embedding and normalize it to 128 dimensions.

        Args:
            text: The input string to embed.

        Returns:
            A list of 128 floats.

        Raises:
            LLMError: On API or network failure.
        """
        if not text or not text.strip():
            logger.warning("Empty text passed to embed(); returning zero vector.")
            return normalize_embedding([], target_dim=128)

        try:
            response = self._client.embeddings.create(
                model=self.embedding_model,
                input=text,
            )
        except Exception as exc:
            raise LLMError(f"OpenAI embedding request failed: {exc}") from exc

        raw_vec = response.data[0].embedding
        return normalize_embedding(raw_vec, target_dim=128)

    # ------------------------------------------------------------------
    # name
    # ------------------------------------------------------------------

    def name(self) -> str:
        """Return a human-readable backend identifier."""
        return f"OpenAI ({self.model})"
