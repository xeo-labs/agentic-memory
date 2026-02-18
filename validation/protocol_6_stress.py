"""Protocol 6: Stress Test

Tests brain growth and integrity under sustained load.

Procedure:
    Sessions 1-20: Each session tells the agent a unique fact.
    After each session, verify the brain's node count grew.
    Session 21: Spot-check 5 randomly selected facts.
    Final: Validate brain integrity via brain.info().
"""

from __future__ import annotations

import random
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

PROTOCOL_NAME = "protocol_6_stress"

# 20 distinct facts for the stress test.
STRESS_FACTS: list[tuple[str, str]] = [
    ("My name is Zara", "Zara"),
    ("I work at Google", "Google"),
    ("I live in San Francisco", "San Francisco"),
    ("My favourite language is Go", "Go"),
    ("I have a dog named Buddy", "Buddy"),
    ("My favourite editor is Neovim", "Neovim"),
    ("I use Kubernetes for container orchestration", "Kubernetes"),
    ("My favourite database is Redis", "Redis"),
    ("I graduated from Stanford", "Stanford"),
    ("I enjoy surfing in my free time", "surfing"),
    ("My favourite framework is Django", "Django"),
    ("I use GitHub for version control", "GitHub"),
    ("My operating system is Arch Linux", "Arch"),
    ("I drink oat milk lattes every morning", "oat milk"),
    ("My monitor is an LG Ultrafine 5K", "Ultrafine"),
    ("I type on a ZSA Moonlander keyboard", "Moonlander"),
    ("My favourite cloud provider is AWS", "AWS"),
    ("I listen to jazz while coding", "jazz"),
    ("I read Hacker News every day", "Hacker News"),
    ("My car is a Rivian R1T", "Rivian"),
]

# Use a fixed seed so spot-checks are reproducible.
SPOT_CHECK_SEED = 42
SPOT_CHECK_COUNT = 5


def run(
    amem_binary: str | None = None,
    use_mock: bool = True,
) -> tuple[str, bool, str]:
    """Execute Protocol 6: Stress Test.

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

        prev_node_count = 0
        growth_failures = 0

        # ------------------------------------------------------------------
        # Sessions 1-20: Seed 20 unique facts and track growth
        # ------------------------------------------------------------------
        for idx, (message, keyword) in enumerate(STRESS_FACTS, start=1):
            session = ValidationSession(brain, llm, session_id=idx)
            resp = session.send(message)

            # Check brain growth.
            try:
                info = brain.info()
                current_count = info.node_count
                grew = current_count > prev_node_count
                if not grew:
                    growth_failures += 1
                    details_lines.append(
                        f"Session {idx:2d}: node_count={current_count} "
                        f"(DID NOT GROW from {prev_node_count})"
                    )
                prev_node_count = current_count
            except Exception as exc:
                details_lines.append(
                    f"Session {idx:2d}: could not read brain info: {exc}"
                )

        details_lines.append(
            f"After 20 sessions: total nodes = {prev_node_count}, "
            f"growth failures = {growth_failures}"
        )

        if growth_failures > 5:
            details_lines.append(
                f"FAIL: Too many sessions without growth ({growth_failures}/20)"
            )
            passed = False

        # ------------------------------------------------------------------
        # Spot-check: 5 random facts from the 20
        # ------------------------------------------------------------------
        rng = random.Random(SPOT_CHECK_SEED)
        spot_indices = rng.sample(range(len(STRESS_FACTS)), SPOT_CHECK_COUNT)
        details_lines.append(
            f"Spot-checking facts from sessions: "
            f"{[i + 1 for i in spot_indices]}"
        )

        spot_pass = 0
        for i in spot_indices:
            message, keyword = STRESS_FACTS[i]
            found = assert_brain_contains(brain, [keyword])
            if found:
                spot_pass += 1
            details_lines.append(
                f"  Spot-check session {i + 1}: "
                f"brain contains '{keyword}': {found}"
            )

        details_lines.append(
            f"Spot-check: {spot_pass}/{SPOT_CHECK_COUNT} facts found in brain"
        )

        if spot_pass < 3:
            details_lines.append(
                f"FAIL: Only {spot_pass}/{SPOT_CHECK_COUNT} spot-checks passed"
            )
            passed = False

        # ------------------------------------------------------------------
        # Brain integrity check
        # ------------------------------------------------------------------
        try:
            info = brain.info()
            details_lines.append(
                f"Brain integrity: version={info.version}, "
                f"nodes={info.node_count}, "
                f"edges={info.edge_count}, "
                f"sessions={info.session_count}, "
                f"facts={info.facts}, "
                f"decisions={info.decisions}, "
                f"corrections={info.corrections}, "
                f"file_size={info.file_size_bytes} bytes"
            )

            if info.node_count == 0:
                details_lines.append("FAIL: Brain has zero nodes after stress test")
                passed = False
            else:
                details_lines.append("Brain integrity check passed")
        except Exception as exc:
            details_lines.append(f"FAIL: Brain integrity check failed: {exc}")
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
