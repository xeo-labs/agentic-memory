"""Cognitive event extraction from conversation exchanges.

Ported and refined from Phase 7A. Uses an LLM to identify facts,
decisions, inferences, and skills from conversation text.
"""

from __future__ import annotations

import logging
from dataclasses import dataclass, field
from typing import Any

from agentic_memory.models import EventType
from agentic_memory.integrations.base import ChatMessage, LLMProvider

logger = logging.getLogger(__name__)

# The extraction prompt. Asks the LLM to return structured JSON.
# Uses doubled braces {{ }} for literal braces in the template.
_EXTRACTION_PROMPT = """Analyze this conversation exchange and extract cognitive events.

Return a JSON object with this exact structure:
{{
    "events": [
        {{
            "type": "fact|decision|inference|skill",
            "content": "clear, concise description",
            "confidence": 0.0-1.0,
            "relationships": []
        }}
    ],
    "corrections": [
        {{
            "old_content": "what was previously believed",
            "new_content": "the corrected information",
            "confidence": 0.9
        }}
    ],
    "session_summary": "one-line summary of this exchange"
}}

Event types:
- fact: Something learned about the user or world (e.g., "User's name is Alice")
- decision: A choice the agent made (e.g., "Recommended Python for the project")
- inference: A conclusion drawn from facts (e.g., "User is likely a senior developer")
- skill: A learned procedure or preference (e.g., "User prefers code examples over explanations")

Rules:
- Only extract genuinely new information
- Set confidence based on how certain the information is
- If the user corrects something, add it to "corrections"
- Return empty lists if no events or corrections are found
- Keep content concise but complete

Existing memories (for context):
{existing_memories}

User message:
{user_message}

Assistant response:
{assistant_response}

Return ONLY the JSON object, no other text."""


@dataclass
class ExtractedEvent:
    """An event extracted from a conversation.

    Attributes:
        type: The event type.
        content: Extracted content text.
        confidence: Confidence level, 0.0 to 1.0.
        relationships: Optional relationship annotations.
    """
    type: EventType
    content: str
    confidence: float
    relationships: list[dict[str, Any]] = field(default_factory=list)


@dataclass
class ExtractionResult:
    """Result of cognitive event extraction.

    Attributes:
        events: List of extracted events.
        corrections: List of corrections (old/new content pairs).
        summary: One-line summary of the exchange.
    """
    events: list[ExtractedEvent]
    corrections: list[dict[str, Any]]
    summary: str


def extract_events(
    provider: LLMProvider,
    user_message: str,
    assistant_response: str,
    existing_memories: str = "",
) -> ExtractionResult:
    """Extract cognitive events from a conversation exchange.

    Uses the LLM to identify facts, decisions, inferences, and skills
    from the conversation.

    Args:
        provider: LLM provider to use for extraction.
        user_message: The user's message.
        assistant_response: The assistant's response.
        existing_memories: Summary of existing memories for context.

    Returns:
        ExtractionResult with extracted events and corrections.
    """
    prompt = _EXTRACTION_PROMPT.format(
        existing_memories=existing_memories or "(none)",
        user_message=user_message,
        assistant_response=assistant_response,
    )

    messages = [
        ChatMessage(role="user", content=prompt),
    ]

    try:
        data = provider.chat_json(messages)
    except Exception as e:
        logger.warning("Extraction failed: %s", e)
        return ExtractionResult(events=[], corrections=[], summary="")

    # Parse events
    events: list[ExtractedEvent] = []
    for raw_event in data.get("events", []):
        try:
            event_type = EventType(raw_event.get("type", "fact"))
            events.append(ExtractedEvent(
                type=event_type,
                content=raw_event.get("content", ""),
                confidence=float(raw_event.get("confidence", 0.8)),
                relationships=raw_event.get("relationships", []),
            ))
        except (ValueError, KeyError) as e:
            logger.debug("Skipping malformed event: %s (%s)", raw_event, e)
            continue

    corrections = data.get("corrections", [])
    summary = data.get("session_summary", data.get("summary", ""))

    return ExtractionResult(
        events=events,
        corrections=corrections,
        summary=summary,
    )
