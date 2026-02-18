"""Cognitive event extraction from conversations.

This module sends conversation turns through an LLM to identify structured
cognitive events -- facts, decisions, inferences, skills, and corrections --
that should be persisted in the agent's long-term memory graph.

Memory formation must NEVER crash the agent.  Every public function in this
module catches all exceptions and returns a safe default so that a transient
LLM error or malformed JSON response degrades gracefully instead of
propagating upward.
"""

from __future__ import annotations

import json
import logging
import re
from dataclasses import dataclass, field
from typing import Any

from amem_agent.llm.base import Message

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Extraction prompt
# ---------------------------------------------------------------------------

EXTRACTION_SYSTEM_PROMPT: str = """\
You are a cognitive memory analyst.  Your job is to analyze a conversation
turn between a user and an AI assistant and extract structured memory events.

For every turn you MUST output valid JSON with exactly this schema:

{
  "events": [
    {
      "event_type": "<fact|decision|inference|skill|correction>",
      "content": "<concise description of what was learned or decided>",
      "confidence": <float 0.0-1.0>,
      "relationships": [
        {"target_description": "<related memory description>", "edge_type": "<supports|contradicts|extends|requires>", "weight": <float 0.0-1.0>}
      ]
    }
  ],
  "corrections": [
    {
      "old_description": "<description of the outdated or incorrect memory>",
      "new_content": "<the corrected information>",
      "confidence": <float 0.0-1.0>
    }
  ],
  "session_summary": "<one-sentence summary of this conversation turn>"
}

Event type definitions:
- **fact**: An objective piece of information stated or confirmed by the user
  (e.g. name, preference, technical detail).
- **decision**: A choice the user or assistant explicitly made during the
  conversation (e.g. "we will use PostgreSQL").
- **inference**: Something that can be reasonably inferred but was not stated
  directly (lower confidence than facts).
- **skill**: A capability, workflow, or technique demonstrated or discussed.
- **correction**: The user corrected a previously stated fact or assumption.
  List these in the "corrections" array so the old memory can be superseded.

Guidelines:
- Be selective.  Only extract events that would be valuable to remember in
  future conversations.  Ignore small talk and filler.
- Confidence reflects how certain you are that the extracted event is accurate
  and worth remembering (1.0 = absolutely certain, 0.5 = plausible guess).
- If there is nothing worth remembering, return empty arrays.
- Relationships link a new event to *existing* memories that are provided in
  the context.  Use the target_description to reference them.
- Output ONLY valid JSON.  No markdown fences, no commentary.
"""

EXTRACTION_USER_TEMPLATE: str = """\
## User message
{user_message}

## Assistant response
{assistant_response}

## Existing memories (for relationship linking)
{existing_memories_summary}

Analyze the above conversation turn and extract cognitive events as JSON.\
"""


# ---------------------------------------------------------------------------
# Data classes
# ---------------------------------------------------------------------------

@dataclass(frozen=True, slots=True)
class ExtractedEvent:
    """A single cognitive event extracted from a conversation turn.

    Attributes:
        event_type: One of ``fact``, ``decision``, ``inference``, ``skill``,
            or ``correction``.
        content: Human-readable description of the event.
        confidence: Float in ``[0, 1]`` indicating extraction confidence.
        relationships: Links to other memories, each a dict with keys
            ``target_description``, ``edge_type``, and ``weight``.
    """

    event_type: str
    content: str
    confidence: float = 0.8
    relationships: list[dict[str, Any]] = field(default_factory=list)


@dataclass(frozen=True, slots=True)
class ExtractedCorrection:
    """A correction that supersedes an earlier memory.

    Attributes:
        old_description: Description of the outdated memory to supersede.
        new_content: The corrected information.
        confidence: Float in ``[0, 1]`` indicating correction confidence.
    """

    old_description: str
    new_content: str
    confidence: float = 0.9


@dataclass(slots=True)
class ExtractionResult:
    """Aggregated result of a single extraction pass.

    Attributes:
        events: Extracted cognitive events.
        corrections: Corrections to existing memories.
        session_summary: One-sentence summary of the conversation turn.
    """

    events: list[ExtractedEvent] = field(default_factory=list)
    corrections: list[ExtractedCorrection] = field(default_factory=list)
    session_summary: str = ""


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def format_existing_memories(nodes: list[dict[str, Any]]) -> str:
    """Format a list of memory nodes as a numbered summary string.

    Each node dict is expected to have at least ``id``, ``event_type``,
    ``content``, and ``confidence`` keys.  Missing keys are replaced with
    sensible defaults.

    Args:
        nodes: Memory node dicts as returned by ``brain.search()``.

    Returns:
        A numbered list like::

            1. [ID:42] FACT: User prefers dark mode (confidence: 95%)
            2. [ID:7]  DECISION: Use PostgreSQL for persistence (confidence: 88%)

        Returns ``"(no existing memories)"`` when *nodes* is empty or ``None``.
    """
    if not nodes:
        return "(no existing memories)"

    lines: list[str] = []
    for idx, node in enumerate(nodes, start=1):
        try:
            node_id = node.get("id", "?")
            event_type = str(node.get("event_type", "unknown")).upper()
            content = node.get("content", "<no content>")
            confidence = node.get("confidence", 0.0)
            confidence_pct = int(round(float(confidence) * 100))
            lines.append(
                f"{idx}. [ID:{node_id}] {event_type}: {content} "
                f"(confidence: {confidence_pct}%)"
            )
        except Exception:
            # A single malformed node should not break the whole summary.
            logger.debug("Skipping malformed memory node at index %d", idx)
            continue

    return "\n".join(lines) if lines else "(no existing memories)"


def find_best_match(
    description: str,
    candidates: list[dict[str, Any]],
) -> dict[str, Any] | None:
    """Find the best matching memory node using simple keyword overlap.

    This is intentionally a lightweight heuristic -- it tokenises both the
    *description* and each candidate's ``content`` into lowercased words and
    picks the candidate with the highest Jaccard-like overlap.

    Args:
        description: A short natural-language description of the target memory.
        candidates: List of memory node dicts (must contain ``content``).

    Returns:
        The best-matching node dict, or ``None`` when *candidates* is empty or
        no candidate shares any keywords with *description*.
    """
    if not description or not candidates:
        return None

    # Simple stop-words to filter out noise.
    _STOP_WORDS = frozenset(
        {"the", "a", "an", "is", "are", "was", "were", "be", "been", "being",
         "have", "has", "had", "do", "does", "did", "will", "would", "shall",
         "should", "may", "might", "must", "can", "could", "to", "of", "in",
         "for", "on", "with", "at", "by", "from", "as", "into", "through",
         "during", "before", "after", "about", "between", "and", "but", "or",
         "not", "no", "nor", "so", "yet", "both", "either", "neither", "each",
         "every", "all", "any", "few", "more", "most", "other", "some", "such",
         "than", "too", "very", "just", "that", "this", "these", "those", "it",
         "its", "i", "me", "my", "we", "us", "our", "you", "your", "he", "him",
         "his", "she", "her", "they", "them", "their", "what", "which", "who",
         "whom", "how", "when", "where", "why"}
    )

    def _keywords(text: str) -> set[str]:
        return {w for w in text.lower().split() if w not in _STOP_WORDS and len(w) > 1}

    desc_kw = _keywords(description)
    if not desc_kw:
        return None

    best_node: dict[str, Any] | None = None
    best_score: float = 0.0

    for node in candidates:
        try:
            node_content = node.get("content", "")
            node_kw = _keywords(str(node_content))
            if not node_kw:
                continue
            overlap = len(desc_kw & node_kw)
            union = len(desc_kw | node_kw)
            score = overlap / union if union else 0.0
            if score > best_score:
                best_score = score
                best_node = node
        except Exception:
            continue

    return best_node


# ---------------------------------------------------------------------------
# Regex-based fallback extraction
# ---------------------------------------------------------------------------

# Patterns that identify factual statements in user messages.
# Each pattern has a name (for logging) and a compiled regex that captures
# the factual content.
_FACT_PATTERNS: list[tuple[str, re.Pattern[str]]] = [
    # "My name is X" (explicit name statement — high confidence)
    ("name", re.compile(
        r"\b(?:my name is)\s+([A-Z][a-z]+(?:\s+[A-Z][a-z]+)*)(?=\s+and\b|\.|,|!|$)",
        re.IGNORECASE,
    )),
    # "I live in X" / "I'm from X" / "I'm based in X" / "in X, Y" (at end of profession)
    ("location", re.compile(
        r"\b(?:i live in|i'm from|i am from|i'm based in|i am based in|i reside in)\s+([A-Z][a-z]+(?:[,\s]+[A-Z][a-z]+)*)",
        re.IGNORECASE,
    )),
    # "X in Y" — catch location at end of role description: "engineer in Munich, Germany"
    ("location_in", re.compile(
        r"\b(?:engineer|developer|designer|manager|analyst|scientist|doctor|teacher|professor|architect|consultant|writer|artist)\s+in\s+([A-Z][a-z]+(?:[,\s]+[A-Z][a-z]+)*)",
        re.IGNORECASE,
    )),
    # "I'm a X" / "I am a X" — profession/role
    ("profession", re.compile(
        r"\b(?:i'm a|i am a|i'm an|i am an)\s+(.+?)(?:\.|,|!|\bin\b|$)",
        re.IGNORECASE,
    )),
    # "I specialize in X" / "I've been doing X"
    ("specialization", re.compile(
        r"\b(?:i (?:specialize|specialise) in|i've been (?:doing|working (?:on|in|with)))\s+(.+?)(?:\.|,|!|$)",
        re.IGNORECASE,
    )),
    # "I work at X" / "I work for X" / "my employer is X"
    ("workplace", re.compile(
        r"\b(?:i work (?:at|for)|my (?:employer|company|workplace) is)\s+(.+?)(?:\.|,|$)",
        re.IGNORECASE,
    )),
    # "I prefer X" / "my favourite X is Y" / "I like X"
    ("preference", re.compile(
        r"\b(?:i prefer|my fav(?:ou?rite)?\s+\w+\s+is|i (?:really )?like|i love)\s+(.+?)(?:\.|,|!|$)",
        re.IGNORECASE,
    )),
    # "I use X" / "my tech stack is X" / "we use X"
    ("tech", re.compile(
        r"\b(?:i use|we use|my (?:tech )?stack is|our stack is)\s+(.+?)(?:\.|,|$)",
        re.IGNORECASE,
    )),
    # "I have a X" / "my X is Y"  (e.g. "my dog is named Bella")
    ("possession", re.compile(
        r"\b(?:i have (?:a |an )?)\s*(.+?)(?:\.|,|!|$)",
        re.IGNORECASE,
    )),
    # "I drive a X" / "I own a X" / "I bought a X" / "I now drive X"
    ("vehicle_or_item", re.compile(
        r"\b(?:i (?:now )?(?:drive|own|bought|got|ride)\s+(?:a |an |my )?)\s*(.+?)(?:\.|,|!|$)",
        re.IGNORECASE,
    )),
    # "I studied at X" / "I went to X (school/university)"
    ("education", re.compile(
        r"\b(?:i (?:studied|went to school|went to university|graduated from|attend(?:ed)?))\s+(?:at\s+)?(.+?)(?:\.|,|$)",
        re.IGNORECASE,
    )),
    # "I'm building X" / "I want to build X"
    ("project", re.compile(
        r"\b(?:i'm building|i am building|i want to build|i'm (?:working on|developing))\s+(.+?)(?:\.|,|!|$)",
        re.IGNORECASE,
    )),
    # "I decided to X" / "We decided to X" / "We chose X" / "Let's use X"
    ("decision", re.compile(
        r"\b(?:i decided (?:to|on)|we decided (?:to|on)|we chose|let'?s use|we(?:'re| are) going with|i'll use)\s+(.+?)(?:\.|,|$)",
        re.IGNORECASE,
    )),
    # "Actually, I now X" / "correction: X" (must have correction signal word)
    ("correction", re.compile(
        r"\b(?:actually,?\s+i\s+(?:now|no longer)|correction:?\s*)\s*(.+?)(?:\.|!|$)",
        re.IGNORECASE,
    )),
    # "I'm allergic to X" / health-related
    ("health", re.compile(
        r"\b(?:i'm allergic to|i am allergic to|i'm (?:intolerant|sensitive) to)\s+(.+?)(?:\.|,|!|$)",
        re.IGNORECASE,
    )),
    # "I'm training for X" / "I'm preparing for X"
    ("activity", re.compile(
        r"\b(?:i'm training for|i am training for|i'm preparing for|i run|i play)\s+(.+?)(?:\.|,|!|$)",
        re.IGNORECASE,
    )),
    # "my favourite X is Y" / "my new favourite X is Y"
    ("favourite", re.compile(
        r"\bmy\s+(?:new\s+)?(?:fav(?:ou?rite)?|preferred)\s+(\w+(?:\s+\w+)*)\s+is\s+(.+?)(?:\.|,|!|$)",
        re.IGNORECASE,
    )),
]


def _fallback_extract_facts(user_message: str) -> ExtractionResult:
    """Extract facts from a user message using regex pattern matching.

    This is the last-resort fallback when the LLM cannot produce valid JSON.
    It uses simple patterns to identify factual statements about the user,
    producing :class:`ExtractedEvent` instances with moderate confidence.

    The approach is intentionally conservative -- it is better to extract
    fewer facts with higher confidence than to hallucinate structure.

    Args:
        user_message: The raw user message text.

    Returns:
        An :class:`ExtractionResult` (may be empty if no patterns match).
    """
    if not user_message:
        return ExtractionResult()

    events: list[ExtractedEvent] = []
    seen_contents: set[str] = set()

    for pattern_name, pattern in _FACT_PATTERNS:
        for match in pattern.finditer(user_message):
            # "favourite" pattern has 2 groups
            if pattern_name == "favourite":
                category = match.group(1).strip()
                value = match.group(2).strip().rstrip(".,!?;:")
                if not value or len(value) < 2:
                    continue
                content = value
                fact = f"User's favourite {category} is {value}"
            else:
                content = match.group(1).strip().rstrip(".,!?;:")
                if not content or len(content) < 2:
                    continue

                # Build a more descriptive content string
                if pattern_name == "name":
                    fact = f"User's name is {content}"
                elif pattern_name in ("location", "location_in"):
                    fact = f"User lives in {content}"
                elif pattern_name == "profession":
                    fact = f"User is a {content}"
                elif pattern_name == "specialization":
                    fact = f"User specializes in {content}"
                elif pattern_name == "workplace":
                    fact = f"User works at {content}"
                elif pattern_name == "preference":
                    fact = match.group(0).strip().rstrip(".,!?;:")
                    fact = fact[0].upper() + fact[1:] if fact else content
                elif pattern_name == "tech":
                    fact = f"User's tech stack includes {content}"
                elif pattern_name == "possession":
                    fact = f"User has {content}"
                elif pattern_name == "vehicle_or_item":
                    fact = f"User has {content}"
                elif pattern_name == "education":
                    fact = f"User studied at {content}"
                elif pattern_name == "project":
                    fact = f"User is building {content}"
                elif pattern_name == "decision":
                    fact = f"Decision: {content}"
                elif pattern_name == "correction":
                    fact = content[0].upper() + content[1:] if content else content
                elif pattern_name == "health":
                    fact = f"User is allergic to {content}"
                elif pattern_name == "activity":
                    fact = f"User is training for {content}"
                else:
                    fact = content

            # Deduplicate
            fact_lower = fact.lower()
            if fact_lower in seen_contents:
                continue
            seen_contents.add(fact_lower)

            event_type = "decision" if pattern_name == "decision" else "fact"
            events.append(ExtractedEvent(
                event_type=event_type,
                content=fact,
                confidence=0.7,  # Lower than LLM extraction
                relationships=[],
            ))

    if events:
        logger.info(
            "Fallback regex extraction found %d event(s) from user message.",
            len(events),
        )

    return ExtractionResult(
        events=events,
        corrections=[],
        session_summary="",
    )


# ---------------------------------------------------------------------------
# Main extraction entry point
# ---------------------------------------------------------------------------

def _parse_extraction_response(raw: dict[str, Any]) -> ExtractionResult:
    """Parse the raw JSON dict returned by the LLM into an ExtractionResult.

    Tolerates missing keys, wrong types, and other common LLM quirks.
    """
    events: list[ExtractedEvent] = []
    corrections: list[ExtractedCorrection] = []
    session_summary: str = str(raw.get("session_summary", ""))

    # -- events ---------------------------------------------------------------
    for item in raw.get("events", []):
        try:
            event_type = str(item.get("event_type", "fact")).lower()
            content = str(item.get("content", ""))
            if not content:
                continue
            confidence = float(item.get("confidence", 0.8))
            confidence = max(0.0, min(1.0, confidence))

            relationships: list[dict[str, Any]] = []
            for rel in item.get("relationships", []):
                if isinstance(rel, dict) and "target_description" in rel:
                    relationships.append({
                        "target_description": str(rel["target_description"]),
                        "edge_type": str(rel.get("edge_type", "supports")),
                        "weight": float(rel.get("weight", 0.5)),
                    })

            events.append(ExtractedEvent(
                event_type=event_type,
                content=content,
                confidence=confidence,
                relationships=relationships,
            ))
        except Exception as exc:
            logger.debug("Skipping malformed event entry: %s", exc)
            continue

    # -- corrections ----------------------------------------------------------
    for item in raw.get("corrections", []):
        try:
            old_desc = str(item.get("old_description", ""))
            new_content = str(item.get("new_content", ""))
            if not old_desc or not new_content:
                continue
            confidence = float(item.get("confidence", 0.9))
            confidence = max(0.0, min(1.0, confidence))
            corrections.append(ExtractedCorrection(
                old_description=old_desc,
                new_content=new_content,
                confidence=confidence,
            ))
        except Exception as exc:
            logger.debug("Skipping malformed correction entry: %s", exc)
            continue

    return ExtractionResult(
        events=events,
        corrections=corrections,
        session_summary=session_summary,
    )


def extract_events(
    llm: Any,
    user_message: str,
    assistant_response: str,
    existing_memories: list[dict[str, Any]] | None = None,
) -> ExtractionResult:
    """Extract cognitive events from a single conversation turn.

    Sends the user/assistant exchange along with a summary of existing
    memories to the LLM and parses its structured JSON output into an
    :class:`ExtractionResult`.

    This function **never raises**.  On any failure (network error, bad JSON,
    unexpected schema) it logs a warning and returns an empty
    ``ExtractionResult``.

    Args:
        llm: An LLM backend that exposes ``chat_json(messages)`` accepting
            a list of :class:`Message` objects.
        user_message: The raw user message text.
        assistant_response: The assistant's reply text.
        existing_memories: Optional list of memory node dicts used to ground
            relationship linking.

    Returns:
        An :class:`ExtractionResult` containing extracted events, corrections,
        and a session summary.
    """
    empty = ExtractionResult()

    if not user_message and not assistant_response:
        return empty

    try:
        memories_summary = format_existing_memories(existing_memories or [])

        user_prompt = EXTRACTION_USER_TEMPLATE.format(
            user_message=user_message,
            assistant_response=assistant_response,
            existing_memories_summary=memories_summary,
        )

        messages = [
            Message(role="system", content=EXTRACTION_SYSTEM_PROMPT),
            Message(role="user", content=user_prompt),
        ]

        raw: dict[str, Any] = llm.chat_json(messages)

        if not isinstance(raw, dict):
            logger.warning(
                "Extraction LLM returned non-dict type: %s -- falling back to regex.",
                type(raw).__name__,
            )
            return _fallback_extract_facts(user_message)

        result = _parse_extraction_response(raw)

        # If structured extraction returned nothing, try regex fallback
        if not result.events and not result.corrections:
            fallback = _fallback_extract_facts(user_message)
            if fallback.events:
                logger.info(
                    "Structured extraction returned empty; regex fallback found %d event(s).",
                    len(fallback.events),
                )
                return fallback

        return result

    except json.JSONDecodeError as exc:
        logger.warning("Extraction failed -- invalid JSON from LLM: %s -- using regex fallback.", exc)
        return _fallback_extract_facts(user_message)
    except Exception as exc:
        logger.warning("Extraction failed -- unexpected error: %s -- using regex fallback.", exc)
        return _fallback_extract_facts(user_message)
