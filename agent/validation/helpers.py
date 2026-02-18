"""Validation helpers: MockLLM, ValidationSession, and assertion utilities.

Provides everything needed to run validation protocols without real API keys.
The MockLLM parses memory context from system prompts and echoes relevant
facts back, while also producing structured extraction JSON so that
extract_and_store writes the right events to the brain.
"""

from __future__ import annotations

import re
import sys
import tempfile
from pathlib import Path
from typing import Any

# Ensure the project root is importable.
_PROJECT_ROOT = str(Path(__file__).resolve().parent.parent)
if _PROJECT_ROOT not in sys.path:
    sys.path.insert(0, _PROJECT_ROOT)

from amem_agent.brain import Brain
from amem_agent.llm.base import LLMBackend, LLMResponse, Message
from amem_agent.memory.context import build_memory_context, extract_and_store
from amem_agent.agent.prompts import build_full_system_prompt


# ---------------------------------------------------------------------------
# Default amem binary location (relative to this repo)
# ---------------------------------------------------------------------------

_DEFAULT_AMEM_BINARY = str(
    Path(__file__).resolve().parent.parent.parent / "target" / "release" / "amem"
)


def default_amem_binary() -> str:
    """Return the default path to the amem binary."""
    return _DEFAULT_AMEM_BINARY


# ---------------------------------------------------------------------------
# MockLLM
# ---------------------------------------------------------------------------

class MockLLM(LLMBackend):
    """A deterministic mock LLM that requires no API keys.

    Behaviour:
    - ``chat(messages)``: Scans the system prompt for memory context lines,
      then looks at the user's latest message.  If the message is a question,
      compose a response that includes any relevant facts found in context.
      If the message is a statement, acknowledge and echo it back.
    - ``chat_json(messages)``: Parses the conversation for factual statements
      and returns extraction-format JSON with events and corrections.
    - ``embed(text)``: Returns a 128-dimension zero vector.
    - ``name()``: Returns ``"MockLLM"``.
    """

    def name(self) -> str:  # noqa: D401
        return "MockLLM"

    def embed(self, text: str) -> list[float]:
        return [0.0] * 128

    # ------------------------------------------------------------------
    # chat -- produce a natural-language response
    # ------------------------------------------------------------------

    def chat(self, messages: list[Message]) -> LLMResponse:
        """Build a response that echoes back relevant facts from memory."""
        system_content = ""
        user_content = ""

        for msg in messages:
            if msg.role == "system":
                system_content = msg.content
            elif msg.role == "user":
                user_content = msg.content

        # Extract fact lines from the memory context block.
        memory_facts = self._extract_memory_facts(system_content)

        # Decide what to say.
        response = self._compose_response(user_content, memory_facts)

        return LLMResponse(
            content=response,
            model="MockLLM",
            input_tokens=len(system_content) + len(user_content),
            output_tokens=len(response),
        )

    # ------------------------------------------------------------------
    # chat_json -- produce extraction JSON
    # ------------------------------------------------------------------

    def chat_json(self, messages: list[Message]) -> dict:
        """Parse the conversation turn and return extraction-format JSON."""
        user_content = ""
        for msg in messages:
            if msg.role == "user":
                user_content = msg.content

        events: list[dict[str, Any]] = []
        corrections: list[dict[str, Any]] = []

        # Look for the user message and assistant response inside the
        # extraction prompt template.
        user_msg = self._extract_section(user_content, "## User message", "## Assistant response")
        assistant_msg = self._extract_section(user_content, "## Assistant response", "## Existing memories")

        # Determine if this is a correction ("Actually", "now", "changed").
        correction_patterns = [
            r"(?i)\bactually\b.*\bnow\b",
            r"(?i)\bactually\s+i\b",
            r"(?i)\bno\s*,?\s*i\b.*\bnow\b",
            r"(?i)\bi\s+(?:changed|switched|moved|updated)\b",
            r"(?i)\bactually\b.*\b(?:changed|switched|moved|work)\b",
        ]

        is_correction = any(re.search(p, user_msg) for p in correction_patterns)

        if is_correction:
            # Try to extract old and new values.
            correction_info = self._parse_correction(user_msg)
            if correction_info:
                corrections.append(correction_info)
                # Also store the new fact.
                events.append({
                    "event_type": "fact",
                    "content": correction_info["new_content"],
                    "confidence": 0.95,
                    "relationships": [],
                })
        else:
            # Extract factual statements from the user message.
            extracted = self._extract_facts_from_text(user_msg)
            for fact_text in extracted:
                events.append({
                    "event_type": "fact",
                    "content": fact_text,
                    "confidence": 0.9,
                    "relationships": [],
                })

            # Check for decision patterns.
            decision_patterns = [
                r"(?i)\bwe\s+decided\b",
                r"(?i)\bwe\s+chose\b",
                r"(?i)\bwe\s+will\s+use\b",
                r"(?i)\bdecided\s+to\b",
                r"(?i)\bi\s+prefer\b",
                r"(?i)\blet'?s?\s+go\s+with\b",
            ]
            for pattern in decision_patterns:
                if re.search(pattern, user_msg):
                    # Reclassify as decision.
                    for event in events:
                        if event["event_type"] == "fact":
                            event["event_type"] = "decision"
                    break

        return {
            "events": events,
            "corrections": corrections,
            "session_summary": f"Turn processed: {user_msg[:80]}",
        }

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

    @staticmethod
    def _extract_memory_facts(system_prompt: str) -> list[str]:
        """Pull factual content lines from the memory context block."""
        facts: list[str] = []
        in_memory = False
        for line in system_prompt.splitlines():
            stripped = line.strip()
            if "Memory Context" in stripped or "Relevant Memories" in stripped:
                in_memory = True
                continue
            if in_memory and stripped.startswith("- "):
                # Strip the leading "- " and any metadata like "[FACT]" and
                # confidence annotations.
                content = stripped[2:]
                content = re.sub(r"^\[.*?\]\s*", "", content)
                content = re.sub(r"\s*\*\(confidence:.*?\)\*\s*$", "", content)
                if content:
                    facts.append(content)
        return facts

    @staticmethod
    def _extract_section(text: str, start_marker: str, end_marker: str) -> str:
        """Extract text between two section markers."""
        start = text.find(start_marker)
        if start == -1:
            return text
        start += len(start_marker)
        end = text.find(end_marker, start)
        if end == -1:
            end = len(text)
        return text[start:end].strip()

    @staticmethod
    def _compose_response(user_message: str, memory_facts: list[str]) -> str:
        """Compose a response from the user message and available memory facts."""
        user_lower = user_message.lower()

        # Is the user asking a question?
        is_question = "?" in user_message or any(
            user_lower.startswith(w)
            for w in ["what", "where", "who", "when", "how", "why", "do ", "does ",
                       "did ", "can ", "could ", "is ", "are ", "tell me"]
        )

        if is_question and memory_facts:
            # Search memory facts for relevant ones.
            relevant = []
            # Extract keywords from question (skip common question words).
            question_words = {"what", "where", "who", "when", "how", "why",
                              "do", "does", "did", "can", "could", "is", "are",
                              "the", "a", "an", "my", "your", "tell", "me",
                              "about", "s", "you", "know", "remember", "i"}
            query_words = {
                w.strip("?.,!'\"-").lower()
                for w in user_message.split()
                if w.strip("?.,!'\"-").lower() not in question_words
                and len(w.strip("?.,!'\"-")) > 1
            }

            for fact in memory_facts:
                fact_lower = fact.lower()
                # Check if any query keyword appears in this fact.
                if any(kw in fact_lower for kw in query_words):
                    relevant.append(fact)

            if relevant:
                facts_str = ". ".join(relevant)
                return f"Based on what I know: {facts_str}."
            else:
                # Even if no keyword match, include all facts as context.
                # This handles cases where the question is about a broad topic.
                return f"From what I remember: {'. '.join(memory_facts)}."

        elif is_question:
            return "I don't have any information about that yet."

        else:
            # Statement -- acknowledge it.
            return f"Got it, I've noted that. {user_message}"

    @staticmethod
    def _extract_facts_from_text(text: str) -> list[str]:
        """Extract factual statements from user text."""
        if not text.strip():
            return []

        facts: list[str] = []

        # Patterns that indicate factual personal statements.
        personal_patterns = [
            (r"(?i)my\s+name\s+is\s+(\S+)", lambda m: f"User's name is {m.group(1)}"),
            (r"(?i)i\s+live\s+in\s+(.+?)(?:\.|$)", lambda m: f"User lives in {m.group(1).strip()}"),
            (r"(?i)i\s+work\s+at\s+(.+?)(?:\.|$)", lambda m: f"User works at {m.group(1).strip()}"),
            (r"(?i)i\s+(?:am|'m)\s+(?:a\s+)?(.+?)(?:\.|$)", lambda m: f"User is {m.group(1).strip()}"),
            (r"(?i)i\s+prefer\s+(.+?)(?:\.|$)", lambda m: f"User prefers {m.group(1).strip()}"),
            (r"(?i)i\s+(?:like|love|enjoy)\s+(.+?)(?:\.|$)", lambda m: f"User likes {m.group(1).strip()}"),
            (r"(?i)my\s+(?:pet|cat|dog)\b.*?(?:named|called|is)\s+(.+?)(?:\.|$)", lambda m: f"User's pet is named {m.group(1).strip()}"),
            (r"(?i)my\s+(\w+)\s+is\s+(.+?)(?:\.|$)", lambda m: f"User's {m.group(1)} is {m.group(2).strip()}"),
            (r"(?i)i\s+(?:use|am using)\s+(.+?)(?:\.|$)", lambda m: f"User uses {m.group(1).strip()}"),
            (r"(?i)i\s+(?:have|own)\s+(.+?)(?:\.|$)", lambda m: f"User has {m.group(1).strip()}"),
            (r"(?i)(?:i'?m|i\s+am)\s+building\s+(.+?)(?:\.|$)", lambda m: f"User is building {m.group(1).strip()}"),
        ]

        for pattern, formatter in personal_patterns:
            match = re.search(pattern, text)
            if match:
                try:
                    fact = formatter(match)
                    if fact and len(fact) > 5:
                        facts.append(fact)
                except Exception:
                    pass

        # If no patterns matched, store the whole text as a general fact.
        if not facts and text.strip():
            # Try to clean up the text into a fact.
            cleaned = text.strip().rstrip(".")
            if len(cleaned) > 5:
                facts.append(cleaned)

        # Also look for technical decisions embedded in longer text.
        tech_patterns = [
            (r"(?i)(?:use|chose|using|decided\s+(?:to\s+)?use)\s+(\w+)\s+(?:because|for|due\s+to)\s+(.+?)(?:\.|$)",
             lambda m: f"Chose {m.group(1)} because {m.group(2).strip()}"),
        ]
        for pattern, formatter in tech_patterns:
            match = re.search(pattern, text)
            if match:
                try:
                    facts.append(formatter(match))
                except Exception:
                    pass

        return facts

    @staticmethod
    def _parse_correction(text: str) -> dict[str, Any] | None:
        """Parse a correction statement into old/new values."""
        # Pattern: "Actually I now work at X" (implies old was different).
        patterns = [
            # "Actually I now work at X" -- old = "work at" context, new = "work at X"
            (r"(?i)actually\s+i\s+now\s+work\s+at\s+(.+?)(?:\.|$)",
             lambda m: {
                 "old_description": "User works at",
                 "new_content": f"User now works at {m.group(1).strip()}",
                 "confidence": 0.95,
             }),
            # "Actually I now live in X"
            (r"(?i)actually\s+i\s+now\s+live\s+in\s+(.+?)(?:\.|$)",
             lambda m: {
                 "old_description": "User lives in",
                 "new_content": f"User now lives in {m.group(1).strip()}",
                 "confidence": 0.95,
             }),
            # "Actually my name is X"
            (r"(?i)actually\s+my\s+name\s+is\s+(.+?)(?:\.|$)",
             lambda m: {
                 "old_description": "User's name is",
                 "new_content": f"User's name is {m.group(1).strip()}",
                 "confidence": 0.95,
             }),
            # Generic "Actually I <verb> <object>"
            (r"(?i)actually\s+i\s+(?:now\s+)?(.+?)(?:\.|$)",
             lambda m: {
                 "old_description": "Previous user information",
                 "new_content": f"User {m.group(1).strip()}",
                 "confidence": 0.85,
             }),
        ]

        for pattern, formatter in patterns:
            match = re.search(pattern, text)
            if match:
                try:
                    return formatter(match)
                except Exception:
                    pass

        return None


# ---------------------------------------------------------------------------
# ValidationSession
# ---------------------------------------------------------------------------

class ValidationSession:
    """Simulates a single agent session programmatically.

    Wraps the brain, LLM, and memory pipeline so that each call to
    :meth:`send` goes through the full memory-augmented conversation
    cycle: build context -> call LLM -> extract and store.

    Args:
        brain: An initialised :class:`Brain` instance.
        llm: An LLM backend (real or :class:`MockLLM`).
        session_id: The integer session ID for this session.
    """

    def __init__(self, brain: Brain, llm: LLMBackend, session_id: int) -> None:
        self.brain = brain
        self.llm = llm
        self.session_id = session_id

    def send(self, user_message: str) -> str:
        """Run one full turn: build context, call LLM, extract events.

        Args:
            user_message: The user's input text.

        Returns:
            The assistant's response text.
        """
        # 1. Build memory context from existing brain state.
        memory_context = build_memory_context(
            brain=self.brain,
            session_id=self.session_id,
            user_message=user_message,
            llm=self.llm,
        )

        # 2. Assemble system prompt.
        system_prompt = build_full_system_prompt(memory_context=memory_context)

        # 3. Call the LLM.
        messages = [
            Message(role="system", content=system_prompt),
            Message(role="user", content=user_message),
        ]
        llm_response = self.llm.chat(messages)
        assistant_response = llm_response.content

        # 4. Extract and store cognitive events from this turn.
        extract_and_store(
            brain=self.brain,
            llm=self.llm,
            user_message=user_message,
            assistant_response=assistant_response,
            session_id=self.session_id,
        )

        return assistant_response


# ---------------------------------------------------------------------------
# Assertion helpers
# ---------------------------------------------------------------------------

def assert_brain_contains(brain: Brain, keywords: list[str]) -> bool:
    """Check if the brain contains nodes whose content matches all keywords.

    Searches all node types and checks if any single node's content
    contains ALL specified keywords (case-insensitive).

    Args:
        brain: The brain to search.
        keywords: List of keyword strings that must all appear in at
            least one node's content.

    Returns:
        ``True`` if at least one node contains all keywords, ``False``
        otherwise.
    """
    try:
        nodes = brain.search(sort="recent", limit=200)
        for node in nodes:
            content = str(node.get("content", "")).lower()
            if all(kw.lower() in content for kw in keywords):
                return True
        return False
    except Exception:
        return False


def assert_response_contains(response: str, keywords: list[str]) -> bool:
    """Check if a response string contains ALL specified keywords.

    Args:
        response: The assistant response text.
        keywords: Keywords that must all be present (case-insensitive).

    Returns:
        ``True`` if all keywords are found, ``False`` otherwise.
    """
    response_lower = response.lower()
    return all(kw.lower() in response_lower for kw in keywords)


def print_result(protocol_name: str, passed: bool, details: str) -> None:
    """Print a formatted validation result line.

    Args:
        protocol_name: Short name for the protocol (e.g. ``"basic_recall"``).
        passed: Whether the protocol passed.
        details: Human-readable summary of what happened.
    """
    status = "PASS" if passed else "FAIL"
    marker = "[+]" if passed else "[-]"
    print(f"  {marker} {protocol_name}: {status}")
    if details:
        for line in details.strip().splitlines():
            print(f"      {line}")


# ---------------------------------------------------------------------------
# Temp brain helper
# ---------------------------------------------------------------------------

def create_temp_brain(amem_binary: str | None = None) -> Brain:
    """Create a temporary brain file for validation.

    Args:
        amem_binary: Path to the amem binary.  Uses default if ``None``.

    Returns:
        An initialised :class:`Brain` backed by a temp file.
    """
    binary = amem_binary or default_amem_binary()
    tmp = tempfile.mktemp(suffix=".amem", prefix="validation_")
    brain = Brain(brain_path=tmp, amem_binary=binary)
    brain.ensure_exists()
    return brain


def cleanup_brain(brain: Brain) -> None:
    """Remove the brain file and its session counter.

    Args:
        brain: The brain whose backing file should be deleted.
    """
    try:
        path = Path(brain.brain_path)
        if path.exists():
            path.unlink()
        # Also clean up the session counter file if present.
        counter = path.parent / ".amem-session-counter"
        if counter.exists():
            counter.unlink()
    except Exception:
        pass
