"""Feature vector generation for memory content.

Provides a single public function :func:`generate_embedding` that turns
arbitrary text content into a fixed-dimension float vector suitable for
storage in the brain's vector index.

The function delegates to the LLM wrapper's ``embed`` method and normalises
the result to the brain's configured dimensionality.  On any failure it
returns a zero vector so that downstream code always receives a list of
the expected length.

Memory formation must NEVER crash the agent.  All errors are caught and
logged.
"""

from __future__ import annotations

import logging
import math
from typing import Any, Protocol

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# LLM protocol
# ---------------------------------------------------------------------------

class EmbeddingLLMProtocol(Protocol):
    """Minimal interface needed for embedding generation."""

    def embed(self, content: str) -> list[float]:
        """Return a raw embedding vector for *content*."""
        ...


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _normalize_to_dimension(
    raw: list[float],
    target_dim: int,
) -> list[float]:
    """Normalise *raw* to exactly *target_dim* floats.

    Strategy:
    - If *raw* is longer than *target_dim*, truncate to the first
      *target_dim* elements.
    - If *raw* is shorter, pad with ``0.0`` on the right.
    - Finally, L2-normalise the vector so that its Euclidean length is 1.0
      (unless the vector is all zeros).

    Args:
        raw: The original embedding vector of arbitrary length.
        target_dim: Desired output dimensionality.

    Returns:
        A list of *target_dim* floats.
    """
    # Truncate or pad
    if len(raw) >= target_dim:
        vec = raw[:target_dim]
    else:
        vec = raw + [0.0] * (target_dim - len(raw))

    # L2 normalise
    magnitude = math.sqrt(sum(v * v for v in vec))
    if magnitude > 0.0:
        vec = [v / magnitude for v in vec]

    return vec


# ---------------------------------------------------------------------------
# Public API
# ---------------------------------------------------------------------------

def generate_embedding(
    llm: EmbeddingLLMProtocol,
    content: str,
    brain_dimension: int = 128,
) -> list[float]:
    """Generate a fixed-dimension embedding vector for *content*.

    Calls ``llm.embed(content)`` to obtain a raw float vector, then
    normalises it (truncate/pad + L2-norm) to *brain_dimension*.

    This function **never raises**.  On any failure (network error, missing
    ``embed`` method, unexpected return type) it logs a warning and returns
    a zero vector of the requested dimension.

    Args:
        llm: An object that exposes an ``embed(content) -> list[float]``
            method.  The raw vector may be of any length.
        content: The text to embed.
        brain_dimension: The target dimensionality required by the brain's
            vector index.  Defaults to ``128``.

    Returns:
        A list of *brain_dimension* floats.  Guaranteed to be exactly
        *brain_dimension* elements long.
    """
    zero_vector: list[float] = [0.0] * brain_dimension

    if not content or not content.strip():
        logger.debug("Empty content -- returning zero vector.")
        return zero_vector

    try:
        raw: Any = llm.embed(content)

        # Validate that the LLM returned something list-like of numbers.
        if not isinstance(raw, (list, tuple)):
            logger.warning(
                "llm.embed returned unexpected type %s -- returning zero vector.",
                type(raw).__name__,
            )
            return zero_vector

        if len(raw) == 0:
            logger.warning("llm.embed returned empty vector -- returning zero vector.")
            return zero_vector

        # Coerce all elements to float (guards against numpy scalars, etc.)
        try:
            float_vec: list[float] = [float(x) for x in raw]
        except (TypeError, ValueError) as exc:
            logger.warning(
                "Could not convert embedding elements to float: %s", exc
            )
            return zero_vector

        return _normalize_to_dimension(float_vec, brain_dimension)

    except AttributeError:
        logger.warning(
            "LLM object has no 'embed' method -- returning zero vector."
        )
        return zero_vector
    except Exception as exc:
        logger.warning("Embedding generation failed: %s", exc)
        return zero_vector
