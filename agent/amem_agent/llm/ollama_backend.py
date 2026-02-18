"""
Ollama LLM backend.

Connects to a locally running `Ollama <https://ollama.com>`_ instance via
its OpenAI-compatible ``/v1`` endpoint.  The ``openai`` Python SDK is
reused as the HTTP client.
"""

from __future__ import annotations

import json
import logging

from .base import LLMBackend, LLMError, LLMResponse, Message, normalize_embedding, sanitize_json_text

logger = logging.getLogger(__name__)

_MAX_TOKENS = 4096


class OllamaBackend(LLMBackend):
    """LLM backend for locally hosted models served by Ollama.

    On initialisation the backend verifies that the Ollama server is
    reachable by issuing a lightweight ``models.list()`` call.  If the
    server is not running a :class:`LLMError` is raised immediately.

    Args:
        base_url: Root URL of the Ollama server.
            Defaults to ``"http://localhost:11434"``.
        model: Model identifier for chat completions.
            Defaults to ``"llama3.2"``.
        embedding_model: Model identifier for text embeddings.
            Defaults to ``"nomic-embed-text"``.

    Raises:
        LLMError: If the ``openai`` package is not installed or if the
            Ollama server is unreachable.
    """

    def __init__(
        self,
        base_url: str = "http://localhost:11434",
        model: str = "llama3.2",
        embedding_model: str = "nomic-embed-text",
    ) -> None:
        try:
            import openai  # noqa: F811
        except ImportError as exc:
            raise LLMError(
                "The 'openai' package is required for OllamaBackend. "
                "Install it with: pip install openai"
            ) from exc

        self.model = model
        self.embedding_model = embedding_model
        self._base_url = base_url.rstrip("/")

        # Ollama exposes an OpenAI-compatible API at /v1.
        self._client = openai.OpenAI(
            base_url=f"{self._base_url}/v1",
            api_key="ollama",  # Ollama does not require a real key.
        )

        # Verify that the server is reachable.
        self._check_server()

        logger.info(
            "OllamaBackend initialised with base_url=%s, model=%s, embedding_model=%s",
            self._base_url,
            self.model,
            self.embedding_model,
        )

    # ------------------------------------------------------------------
    # chat
    # ------------------------------------------------------------------

    def chat(self, messages: list[Message]) -> LLMResponse:
        """Send *messages* to the Ollama-hosted model.

        Args:
            messages: Ordered conversation history.

        Returns:
            An :class:`LLMResponse` populated from the API response.

        Raises:
            LLMError: On any network or server error.
        """
        api_messages = [{"role": m.role, "content": m.content} for m in messages]

        try:
            response = self._client.chat.completions.create(
                model=self.model,
                messages=api_messages,
                max_tokens=_MAX_TOKENS,
            )
        except Exception as exc:
            raise LLMError(f"Ollama chat request failed: {exc}") from exc

        choice = response.choices[0]
        usage = response.usage
        return LLMResponse(
            content=choice.message.content or "",
            model=response.model or self.model,
            input_tokens=usage.prompt_tokens if usage else 0,
            output_tokens=usage.completion_tokens if usage else 0,
        )

    # ------------------------------------------------------------------
    # chat_json
    # ------------------------------------------------------------------

    def chat_json(self, messages: list[Message]) -> dict:
        """Request a JSON response from the Ollama-hosted model.

        Because Ollama's OpenAI-compatible endpoint does not always
        support ``response_format``, an explicit instruction to respond
        in JSON is appended to the conversation instead.  If the first
        attempt fails to parse, the response is sanitised (markdown
        fences stripped, leading/trailing prose removed) and retried.
        If that still fails, a stricter retry prompt is sent.

        Args:
            messages: Ordered conversation history.

        Returns:
            Parsed JSON as a ``dict``.

        Raises:
            LLMError: If the model output cannot be parsed as JSON after all
                recovery attempts.
        """
        json_instruction = Message(
            role="user",
            content=(
                "Respond with valid JSON only. "
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
                "JSON parse failed after sanitisation for Ollama model=%s; retrying with stricter prompt.",
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
                    "No explanation, no markdown, no code fences, no extra text. "
                    "Just the raw JSON object."
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
                f"Ollama model did not return valid JSON after retry: {response.content!r}"
            ) from exc

    # ------------------------------------------------------------------
    # embed
    # ------------------------------------------------------------------

    def embed(self, text: str) -> list[float]:
        """Create a text embedding via Ollama and normalize to 128 dimensions.

        Args:
            text: The input string to embed.

        Returns:
            A list of 128 floats.

        Raises:
            LLMError: On network or server failure.
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
            raise LLMError(f"Ollama embedding request failed: {exc}") from exc

        raw_vec = response.data[0].embedding
        return normalize_embedding(raw_vec, target_dim=128)

    # ------------------------------------------------------------------
    # name
    # ------------------------------------------------------------------

    def name(self) -> str:
        """Return a human-readable backend identifier."""
        return f"Ollama ({self.model})"

    # ------------------------------------------------------------------
    # Helpers
    # ------------------------------------------------------------------

    def _check_server(self) -> None:
        """Verify that the Ollama server is reachable.

        Raises:
            LLMError: If the server does not respond.
        """
        try:
            self._client.models.list()
        except Exception as exc:
            raise LLMError(
                f"Cannot reach Ollama server at {self._base_url}. "
                "Is Ollama running?  Start it with: ollama serve"
            ) from exc
