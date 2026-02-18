"""Memory engine for the amem-agent.

Re-exports the key classes and functions so that callers can write::

    from amem_agent.memory import (
        ExtractedEvent,
        ExtractedCorrection,
        ExtractionResult,
        extract_events,
        build_memory_context,
        generate_embedding,
    )
"""

from .context import build_memory_context, extract_and_store
from .embeddings import generate_embedding
from .extractor import (
    ExtractedCorrection,
    ExtractedEvent,
    ExtractionResult,
    extract_events,
)

__all__ = [
    "ExtractedEvent",
    "ExtractedCorrection",
    "ExtractionResult",
    "extract_events",
    "build_memory_context",
    "extract_and_store",
    "generate_embedding",
]
