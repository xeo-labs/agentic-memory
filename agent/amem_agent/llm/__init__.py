"""
LLM backend package for **amem-agent**.

Public API
----------
- :func:`create_backend` -- factory that instantiates the right backend
  from a configuration object.
- :class:`~.base.LLMBackend` -- abstract base class.
- :class:`~.base.Message` -- conversation message dataclass.
- :class:`~.base.LLMResponse` -- model response dataclass.
- :class:`~.base.LLMError` -- common exception type.
- :func:`~.base.normalize_embedding` -- vector dimension utility.

Backend implementations
-----------------------
- :class:`~.anthropic_backend.AnthropicBackend`
- :class:`~.openai_backend.OpenAIBackend`
- :class:`~.ollama_backend.OllamaBackend`
"""

from __future__ import annotations

import logging
from typing import Any

from .base import LLMBackend, LLMError, LLMResponse, Message, normalize_embedding, sanitize_json_text

logger = logging.getLogger(__name__)

__all__ = [
    "create_backend",
    "LLMBackend",
    "LLMError",
    "LLMResponse",
    "Message",
    "normalize_embedding",
    "sanitize_json_text",
]


def create_backend(config: Any) -> LLMBackend:
    """Instantiate an LLM backend based on *config*.

    The *config* object must expose a ``backend`` attribute (or key) whose
    value is one of ``"anthropic"``, ``"openai"``, or ``"ollama"``.
    Additional attributes are forwarded to the backend constructor as
    keyword arguments when present.

    Recognised config attributes per backend:

    **anthropic**
        ``api_key``, ``model``

    **openai**
        ``api_key``, ``model``, ``embedding_model``

    **ollama**
        ``base_url``, ``model``, ``embedding_model``

    Args:
        config: A configuration object (dataclass, dict, namespace, ...).
            Must have at least a ``backend`` field.

    Returns:
        A concrete :class:`LLMBackend` instance ready for use.

    Raises:
        ValueError: If ``config.backend`` is not a recognised backend name.
        LLMError: If the backend cannot be initialised (missing key, server
            unreachable, ...).
    """
    backend_name = _get(config, "backend")

    if backend_name == "anthropic":
        from .anthropic_backend import AnthropicBackend

        # Config uses anthropic_api_key; constructor expects api_key.
        kwargs = _collect_kwargs_mapped(config, {
            "anthropic_api_key": "api_key",
            "model": "model",
        })
        logger.info("Creating AnthropicBackend with kwargs=%s", list(kwargs.keys()))
        return AnthropicBackend(**kwargs)

    if backend_name == "openai":
        from .openai_backend import OpenAIBackend

        kwargs = _collect_kwargs_mapped(config, {
            "openai_api_key": "api_key",
            "model": "model",
        })
        logger.info("Creating OpenAIBackend with kwargs=%s", list(kwargs.keys()))
        return OpenAIBackend(**kwargs)

    if backend_name == "ollama":
        from .ollama_backend import OllamaBackend

        kwargs = _collect_kwargs_mapped(config, {
            "ollama_url": "base_url",
            "ollama_model": "model",
        })
        logger.info("Creating OllamaBackend with kwargs=%s", list(kwargs.keys()))
        return OllamaBackend(**kwargs)

    raise ValueError(
        f"Unknown LLM backend: {backend_name!r}. "
        "Supported backends are: 'anthropic', 'openai', 'ollama'."
    )


# ------------------------------------------------------------------
# Internal helpers
# ------------------------------------------------------------------

def _get(obj: Any, key: str) -> Any:
    """Retrieve *key* from *obj* whether it is a dict, namespace, or dataclass."""
    if isinstance(obj, dict):
        return obj[key]
    return getattr(obj, key)


def _collect_kwargs(config: Any, keys: list[str]) -> dict[str, Any]:
    """Build a ``kwargs`` dict from *config* for the given *keys*.

    Only keys that actually exist on *config* (and are not ``None``) are
    included, so that backend constructors fall back to their own defaults
    for anything not explicitly configured.
    """
    kwargs: dict[str, Any] = {}
    for key in keys:
        try:
            value = _get(config, key)
        except (KeyError, AttributeError):
            continue
        if value is not None:
            kwargs[key] = value
    return kwargs


def _collect_kwargs_mapped(
    config: Any, mapping: dict[str, str]
) -> dict[str, Any]:
    """Build a ``kwargs`` dict by mapping config attribute names to constructor
    parameter names.

    Args:
        config: Configuration object.
        mapping: ``{config_attr_name: constructor_param_name}`` pairs.

    Returns:
        A dict suitable for ``**kwargs`` to the backend constructor.
    """
    kwargs: dict[str, Any] = {}
    for config_key, param_name in mapping.items():
        try:
            value = _get(config, config_key)
        except (KeyError, AttributeError):
            continue
        if value is not None:
            kwargs[param_name] = value
    return kwargs
