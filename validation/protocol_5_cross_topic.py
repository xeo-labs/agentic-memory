"""Protocol 5: Cross-Topic Inference

Tests whether the agent can connect related facts from different sessions
to answer a question that requires combining them.

Procedure:
    Session 1: "I'm building a REST API"
    Session 2: "I prefer Python for backend work"
    Session 3: "What language should my API use?" -- should connect Python + API
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

PROTOCOL_NAME = "protocol_5_cross_topic"


def run(
    amem_binary: str | None = None,
    use_mock: bool = True,
) -> tuple[str, bool, str]:
    """Execute Protocol 5: Cross-Topic Inference.

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
        # Session 1: State the project type
        # ------------------------------------------------------------------
        session1 = ValidationSession(brain, llm, session_id=1)
        resp1 = session1.send("I'm building a REST API")
        details_lines.append(f"Session 1 response: {resp1[:120]}")

        brain_has_api = assert_brain_contains(brain, ["API"])
        details_lines.append(f"Brain contains 'API': {brain_has_api}")

        # ------------------------------------------------------------------
        # Session 2: State the language preference
        # ------------------------------------------------------------------
        session2 = ValidationSession(brain, llm, session_id=2)
        resp2 = session2.send("I prefer Python for backend work")
        details_lines.append(f"Session 2 response: {resp2[:120]}")

        brain_has_python = assert_brain_contains(brain, ["Python"])
        details_lines.append(f"Brain contains 'Python': {brain_has_python}")

        # ------------------------------------------------------------------
        # Session 3: Ask a question that requires connecting both facts
        # ------------------------------------------------------------------
        session3 = ValidationSession(brain, llm, session_id=3)
        resp3 = session3.send("What language should my API use?")
        details_lines.append(f"Session 3 response: {resp3[:120]}")

        # The agent should mention Python (the preferred backend language).
        has_python = assert_response_contains(resp3, ["Python"])
        details_lines.append(f"Response contains 'Python': {has_python}")

        if not has_python:
            details_lines.append(
                "FAIL: Agent did not connect Python preference to API question"
            )
            passed = False

        # Bonus check: does the response also reference API context?
        has_api = assert_response_contains(resp3, ["API"])
        details_lines.append(f"Response also contains 'API': {has_api}")
        if has_api:
            details_lines.append(
                "Agent successfully connected cross-topic facts"
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
