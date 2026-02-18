"""Protocol 4: Long-Range Recall

Tests whether the agent retains facts across many sessions.

Procedure:
    Sessions 1-10: Each session tells the agent a distinct personal fact.
    Session 11: Ask about facts from sessions 1, 3, and 5.
    Verify at least 70% recall (i.e. 2 out of 3 facts recovered).
"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from validation.helpers import (
    MockLLM,
    ValidationSession,
    assert_brain_contains,
    assert_response_contains,
    cleanup_brain,
    create_temp_brain,
    default_amem_binary,
    print_result,
)

PROTOCOL_NAME = "protocol_4_long_range"

# Facts seeded in sessions 1-10.  Each tuple is (user_message, keyword_to_check).
SESSION_FACTS: list[tuple[str, str]] = [
    ("My name is Marcus", "Marcus"),                              # session 1
    ("I work at Acme Corp as a software engineer", "Acme"),       # session 2
    ("My favourite programming language is Rust", "Rust"),        # session 3
    ("I have a cat named Whiskers", "Whiskers"),                  # session 4
    ("My tech stack is React, Node.js, and PostgreSQL", "React"), # session 5
    ("I live in Seattle, Washington", "Seattle"),                  # session 6
    ("I enjoy rock climbing on weekends", "climbing"),            # session 7
    ("I graduated from MIT in 2018", "MIT"),                      # session 8
    ("My favourite book is Dune by Frank Herbert", "Dune"),       # session 9
    ("I drive a Tesla Model 3", "Tesla"),                         # session 10
]

# Which sessions to probe in session 11 (0-indexed into SESSION_FACTS).
PROBE_INDICES = [0, 2, 4]  # sessions 1, 3, 5
PROBE_QUESTIONS = [
    ("What is my name?", "Marcus"),
    ("What is my favourite programming language?", "Rust"),
    ("What is my tech stack?", "React"),
]


def run(
    amem_binary: str | None = None,
    use_mock: bool = True,
) -> tuple[str, bool, str]:
    """Execute Protocol 4: Long-Range Recall.

    Args:
        amem_binary: Path to the amem binary.  Uses default if ``None``.
        use_mock: If ``True``, use MockLLM instead of a real LLM backend.

    Returns:
        A tuple of ``(protocol_name, passed, details)``.
    """
    binary = amem_binary or default_amem_binary()
    details_lines: list[str] = []
    passed = True

    brain = None
    try:
        brain = create_temp_brain(binary)
        llm = MockLLM()

        # ------------------------------------------------------------------
        # Sessions 1-10: Seed diverse facts
        # ------------------------------------------------------------------
        for idx, (message, keyword) in enumerate(SESSION_FACTS, start=1):
            session = ValidationSession(brain, llm, session_id=idx)
            resp = session.send(message)
            stored = assert_brain_contains(brain, [keyword])
            details_lines.append(
                f"Session {idx:2d}: sent '{message[:50]}' | "
                f"stored={stored}"
            )
            if not stored:
                details_lines.append(
                    f"  WARN: '{keyword}' not found in brain after session {idx}"
                )

        # ------------------------------------------------------------------
        # Session 11: Probe facts from sessions 1, 3, 5
        # ------------------------------------------------------------------
        recall_count = 0
        total_probes = len(PROBE_QUESTIONS)

        for question, expected_keyword in PROBE_QUESTIONS:
            session = ValidationSession(brain, llm, session_id=11)
            resp = session.send(question)
            recalled = assert_response_contains(resp, [expected_keyword])
            if recalled:
                recall_count += 1
            details_lines.append(
                f"Probe: '{question}' -> "
                f"contains '{expected_keyword}': {recalled}"
            )
            details_lines.append(f"  Response: {resp[:100]}")

        recall_pct = (recall_count / total_probes) * 100 if total_probes > 0 else 0
        details_lines.append(
            f"Recall: {recall_count}/{total_probes} = {recall_pct:.0f}%"
        )

        if recall_pct < 70:
            details_lines.append(
                f"FAIL: Recall {recall_pct:.0f}% is below 70% threshold"
            )
            passed = False
        else:
            details_lines.append(
                f"Recall {recall_pct:.0f}% meets 70% threshold"
            )

    except Exception as exc:
        passed = False
        details_lines.append(f"ERROR: {exc}")

    finally:
        if brain is not None:
            cleanup_brain(brain)

    details = "\n".join(details_lines)
    print_result(PROTOCOL_NAME, passed, details)
    return PROTOCOL_NAME, passed, details


if __name__ == "__main__":
    name, ok, info = run()
    sys.exit(0 if ok else 1)
