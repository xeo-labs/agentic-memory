"""Protocol 1: Basic Recall

Tests whether the agent remembers simple facts across sessions.

Procedure:
    Session 1: Tell the agent "My name is Marcus and I live in Portland"
    Session 2: Ask "What's my name?" -- verify response contains "Marcus"
    Session 3: Ask "Where do I live?" -- verify response contains "Portland"

Uses the real amem CLI but can operate with MockLLM (no API keys needed).
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

PROTOCOL_NAME = "protocol_1_basic_recall"


def run(
    amem_binary: str | None = None,
    use_mock: bool = True,
) -> tuple[str, bool, str]:
    """Execute Protocol 1: Basic Recall.

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
        # Session 1: Tell the agent personal facts
        # ------------------------------------------------------------------
        session1 = ValidationSession(brain, llm, session_id=1)
        resp1 = session1.send("My name is Marcus and I live in Portland")
        details_lines.append(f"Session 1 response: {resp1[:120]}")

        # Verify facts were stored in the brain.
        brain_has_marcus = assert_brain_contains(brain, ["Marcus"])
        brain_has_portland = assert_brain_contains(brain, ["Portland"])
        details_lines.append(
            f"Brain contains 'Marcus': {brain_has_marcus}, "
            f"'Portland': {brain_has_portland}"
        )
        if not brain_has_marcus or not brain_has_portland:
            details_lines.append("FAIL: Facts not stored in brain after session 1")
            passed = False

        # ------------------------------------------------------------------
        # Session 2: Ask about name
        # ------------------------------------------------------------------
        session2 = ValidationSession(brain, llm, session_id=2)
        resp2 = session2.send("What's my name?")
        details_lines.append(f"Session 2 response: {resp2[:120]}")

        name_recalled = assert_response_contains(resp2, ["Marcus"])
        details_lines.append(f"Response contains 'Marcus': {name_recalled}")
        if not name_recalled:
            details_lines.append("FAIL: Agent did not recall 'Marcus' in session 2")
            passed = False

        # ------------------------------------------------------------------
        # Session 3: Ask about location
        # ------------------------------------------------------------------
        session3 = ValidationSession(brain, llm, session_id=3)
        resp3 = session3.send("Where do I live?")
        details_lines.append(f"Session 3 response: {resp3[:120]}")

        location_recalled = assert_response_contains(resp3, ["Portland"])
        details_lines.append(f"Response contains 'Portland': {location_recalled}")
        if not location_recalled:
            details_lines.append("FAIL: Agent did not recall 'Portland' in session 3")
            passed = False

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
