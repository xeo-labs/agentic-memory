"""Detect which LLM backends are available on this machine."""

from __future__ import annotations

import os
from dataclasses import dataclass

from amem_agent.llm.base import LLMBackend, Message


@dataclass
class BackendAvailability:
    name: str                       # "anthropic", "openai", "ollama"
    available: bool                 # Can we actually reach it?
    reason: str | None              # Why unavailable (if not available)
    backend: LLMBackend | None      # Initialized backend (if available)


class BackendDetector:
    """Detect which LLM backends are available on this machine."""

    def detect_all(self) -> list[BackendAvailability]:
        """Check every supported backend. Returns availability for each."""
        results = []
        results.append(self._check_anthropic())
        results.append(self._check_openai())
        results.append(self._check_ollama())
        return results

    def get_available_backends(self) -> list[LLMBackend]:
        """Return only the backends that are actually available."""
        return [r.backend for r in self.detect_all() if r.available]

    def get_available_pairs(self) -> list[tuple[LLMBackend, LLMBackend]]:
        """Return all unique pairs of available backends for cross-testing."""
        available = self.get_available_backends()
        pairs = []
        for i in range(len(available)):
            for j in range(i + 1, len(available)):
                pairs.append((available[i], available[j]))
        return pairs

    # ------------------------------------------------------------------
    # Individual backend checks
    # ------------------------------------------------------------------

    def _check_anthropic(self) -> BackendAvailability:
        """Check if Anthropic API is reachable."""
        api_key = os.environ.get("ANTHROPIC_API_KEY")
        if not api_key:
            return BackendAvailability("anthropic", False, "ANTHROPIC_API_KEY not set", None)
        try:
            from amem_agent.llm.anthropic_backend import AnthropicBackend

            backend = AnthropicBackend(api_key=api_key, model="claude-sonnet-4-20250514")
            response = backend.chat([Message(role="user", content="Say OK")])
            if response and response.content:
                return BackendAvailability("anthropic", True, None, backend)
            return BackendAvailability("anthropic", False, "Empty response", None)
        except Exception as e:
            return BackendAvailability("anthropic", False, str(e), None)

    def _check_openai(self) -> BackendAvailability:
        """Check if OpenAI API is reachable."""
        api_key = os.environ.get("OPENAI_API_KEY")
        if not api_key:
            return BackendAvailability("openai", False, "OPENAI_API_KEY not set", None)
        try:
            from amem_agent.llm.openai_backend import OpenAIBackend

            backend = OpenAIBackend(api_key=api_key, model="gpt-4o")
            response = backend.chat([Message(role="user", content="Say OK")])
            if response and response.content:
                return BackendAvailability("openai", True, None, backend)
            return BackendAvailability("openai", False, "Empty response", None)
        except Exception as e:
            return BackendAvailability("openai", False, str(e), None)

    def _check_ollama(self) -> BackendAvailability:
        """Check if Ollama is running locally."""
        try:
            import httpx

            r = httpx.get("http://localhost:11434/api/tags", timeout=5)
            if r.status_code != 200:
                return BackendAvailability("ollama", False, "Ollama not responding", None)
            models = r.json().get("models", [])
            if not models:
                return BackendAvailability("ollama", False, "No models installed in Ollama", None)

            # Prefer smaller, faster models for validation (avoids 70B+ models).
            # Skip cloud-routed models and pick the smallest truly local model.
            MIN_LOCAL_SIZE = 100_000_000  # ~100MB — real models are larger
            MAX_PREFERRED_SIZE = 10_000_000_000  # ~10GB — avoid huge models
            local_models = [
                m for m in models
                if m.get("size", 0) >= MIN_LOCAL_SIZE and "cloud" not in m["name"]
            ]
            # Sort by size ascending to prefer smaller/faster models
            local_models.sort(key=lambda m: m.get("size", float("inf")))
            # Prefer models under MAX_PREFERRED_SIZE, but fall back to any local model
            preferred = [m for m in local_models if m.get("size", 0) <= MAX_PREFERRED_SIZE]
            if preferred:
                model_name = preferred[0]["name"]
            elif local_models:
                model_name = local_models[0]["name"]
            else:
                model_name = models[0]["name"]

            from amem_agent.llm.ollama_backend import OllamaBackend

            backend = OllamaBackend(model=model_name)
            response = backend.chat([Message(role="user", content="Say OK")])
            if response and response.content:
                return BackendAvailability("ollama", True, None, backend)
            return BackendAvailability("ollama", False, "Empty response", None)
        except Exception as e:
            return BackendAvailability("ollama", False, str(e), None)
