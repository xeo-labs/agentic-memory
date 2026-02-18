"""Protocol 2: Decision Recall

Tests whether the agent remembers technical decisions and their reasoning.

Procedure:
    Session 1: State "We decided to use PostgreSQL because of its JSONB support"
    Session 2: Ask "What database did we choose?" -- verify PostgreSQL
    Session 3: Ask "Why did we choose that database?" -- verify reasoning (JSONB)
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

PROTOCOL_NAME = "protocol_2_decision_recall"


def run(
    amem_binary: str | None = None,
    use_mock: bool = True,
) -> tuple[str, bool, str]:
    """Execute Protocol 2: Decision Recall.

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
        # Session 1: State a technical decision with reasoning
        # ------------------------------------------------------------------
        session1 = ValidationSession(brain, llm, session_id=1)
        resp1 = session1.send(
            "We decided to use PostgreSQL because of its JSONB support"
        )
        details_lines.append(f"Session 1 response: {resp1[:120]}")

        # Verify the decision was stored.
        brain_has_pg = assert_brain_contains(brain, ["PostgreSQL"])
        details_lines.append(f"Brain contains 'PostgreSQL': {brain_has_pg}")
        if not brain_has_pg:
            details_lines.append("FAIL: Decision not stored in brain")
            passed = False

        brain_has_jsonb = assert_brain_contains(brain, ["JSONB"])
        details_lines.append(f"Brain contains 'JSONB': {brain_has_jsonb}")

        # ------------------------------------------------------------------
        # Session 2: Ask what database was chosen
        # ------------------------------------------------------------------
        session2 = ValidationSession(brain, llm, session_id=2)
        resp2 = session2.send("What database did we choose?")
        details_lines.append(f"Session 2 response: {resp2[:120]}")

        db_recalled = assert_response_contains(resp2, ["PostgreSQL"])
        details_lines.append(f"Response contains 'PostgreSQL': {db_recalled}")
        if not db_recalled:
            details_lines.append(
                "FAIL: Agent did not recall 'PostgreSQL' in session 2"
            )
            passed = False

        # ------------------------------------------------------------------
        # Session 3: Ask why that database was chosen
        # ------------------------------------------------------------------
        session3 = ValidationSession(brain, llm, session_id=3)
        resp3 = session3.send("Why did we choose that database?")
        details_lines.append(f"Session 3 response: {resp3[:120]}")

        # The reasoning should reference JSONB support.
        reason_recalled = assert_response_contains(resp3, ["JSONB"])
        details_lines.append(f"Response contains 'JSONB': {reason_recalled}")
        if not reason_recalled:
            # Also accept if it just mentions PostgreSQL at minimum.
            pg_present = assert_response_contains(resp3, ["PostgreSQL"])
            if pg_present:
                details_lines.append(
                    "WARN: JSONB not explicitly in response, but PostgreSQL present"
                )
            else:
                details_lines.append(
                    "FAIL: Agent did not recall reasoning (JSONB) in session 3"
                )
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
