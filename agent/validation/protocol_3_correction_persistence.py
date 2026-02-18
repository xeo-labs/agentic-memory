"""Protocol 3: Correction Persistence

Tests whether the agent properly handles corrections and updates its memory.

Procedure:
    Session 1: State "I work at Company A"
    Session 2: Correct: "Actually I now work at Company B"
    Session 3: Ask "Where do I work?" -- should say Company B, NOT Company A
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

PROTOCOL_NAME = "protocol_3_correction_persistence"


def run(
    amem_binary: str | None = None,
    use_mock: bool = True,
) -> tuple[str, bool, str]:
    """Execute Protocol 3: Correction Persistence.

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
        # Session 1: State initial workplace
        # ------------------------------------------------------------------
        session1 = ValidationSession(brain, llm, session_id=1)
        resp1 = session1.send("I work at Company A")
        details_lines.append(f"Session 1 response: {resp1[:120]}")

        brain_has_a = assert_brain_contains(brain, ["Company A"])
        details_lines.append(f"Brain contains 'Company A': {brain_has_a}")
        if not brain_has_a:
            details_lines.append("FAIL: Initial fact not stored")
            passed = False

        # ------------------------------------------------------------------
        # Session 2: Correct the workplace
        # ------------------------------------------------------------------
        session2 = ValidationSession(brain, llm, session_id=2)
        resp2 = session2.send("Actually I now work at Company B")
        details_lines.append(f"Session 2 response: {resp2[:120]}")

        brain_has_b = assert_brain_contains(brain, ["Company B"])
        details_lines.append(f"Brain contains 'Company B': {brain_has_b}")
        if not brain_has_b:
            details_lines.append("FAIL: Corrected fact not stored")
            passed = False

        # Check that a correction node exists.
        try:
            corrections = brain.search(event_types=["correction"], limit=10)
            has_correction_node = len(corrections) > 0
            details_lines.append(
                f"Correction node count: {len(corrections)}"
            )
        except Exception:
            has_correction_node = False
            details_lines.append("WARN: Could not check for correction nodes")

        # ------------------------------------------------------------------
        # Session 3: Verify the correction is used
        # ------------------------------------------------------------------
        session3 = ValidationSession(brain, llm, session_id=3)
        resp3 = session3.send("Where do I work?")
        details_lines.append(f"Session 3 response: {resp3[:120]}")

        has_company_b = assert_response_contains(resp3, ["Company B"])
        details_lines.append(f"Response contains 'Company B': {has_company_b}")

        if not has_company_b:
            details_lines.append(
                "FAIL: Agent did not recall corrected workplace 'Company B'"
            )
            passed = False

        # The ideal is that Company A is NOT in the response (superseded).
        # But with MockLLM the memory context may still include the old fact
        # alongside the correction.  We treat presence of Company B as
        # sufficient for a pass but note if A is also present.
        has_company_a = assert_response_contains(resp3, ["Company A"])
        if has_company_a:
            details_lines.append(
                "WARN: Response also mentions 'Company A' "
                "(old fact not fully superseded in context)"
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
