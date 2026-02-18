"""Run all validation protocols in sequence and print a summary.

Usage:
    python -m validation.run_all                    # default binary, mock LLM
    python validation/run_all.py                    # same
    python validation/run_all.py /path/to/amem      # custom binary path
"""

from __future__ import annotations

import sys
import time
from pathlib import Path

# Ensure project root is importable.
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from validation.helpers import default_amem_binary

# Import all protocol modules.
from validation import (
    protocol_1_basic_recall,
    protocol_2_decision_recall,
    protocol_3_correction_persistence,
    protocol_4_long_range,
    protocol_5_cross_topic,
    protocol_6_stress,
)

PROTOCOLS = [
    protocol_1_basic_recall,
    protocol_2_decision_recall,
    protocol_3_correction_persistence,
    protocol_4_long_range,
    protocol_5_cross_topic,
    protocol_6_stress,
]


def run_all(
    amem_binary: str | None = None,
    use_mock: bool = True,
) -> list[tuple[str, bool, str]]:
    """Run every validation protocol and return results.

    Args:
        amem_binary: Path to the amem binary.  Uses default if ``None``.
        use_mock: If ``True``, all protocols use MockLLM.

    Returns:
        A list of ``(protocol_name, passed, details)`` tuples.
    """
    binary = amem_binary or default_amem_binary()
    results: list[tuple[str, bool, str]] = []

    print("=" * 60)
    print("  amem-agent Validation Suite")
    print("=" * 60)
    print(f"  Binary : {binary}")
    print(f"  Mode   : {'MockLLM (no API keys)' if use_mock else 'Real LLM'}")
    print("=" * 60)
    print()

    for protocol in PROTOCOLS:
        proto_name = getattr(protocol, "PROTOCOL_NAME", protocol.__name__)
        print(f"--- Running: {proto_name} ---")
        start = time.time()

        try:
            name, ok, details = protocol.run(
                amem_binary=binary,
                use_mock=use_mock,
            )
            elapsed = time.time() - start
            results.append((name, ok, details))
        except Exception as exc:
            elapsed = time.time() - start
            results.append((proto_name, False, f"CRASHED: {exc}"))
            print(f"  [-] {proto_name}: CRASHED -- {exc}")

        print(f"      (elapsed: {elapsed:.2f}s)")
        print()

    # ------------------------------------------------------------------
    # Summary
    # ------------------------------------------------------------------
    total = len(results)
    passed = sum(1 for _, ok, _ in results if ok)
    failed = total - passed

    print("=" * 60)
    print("  SUMMARY")
    print("=" * 60)

    for name, ok, _ in results:
        status = "PASS" if ok else "FAIL"
        marker = "[+]" if ok else "[-]"
        print(f"  {marker} {name}: {status}")

    print()
    print(f"  Total : {total}")
    print(f"  Passed: {passed}")
    print(f"  Failed: {failed}")
    pct = (passed / total * 100) if total > 0 else 0
    print(f"  Rate  : {pct:.0f}%")
    print("=" * 60)

    return results


def main() -> None:
    """Entry point for CLI invocation."""
    # Accept an optional binary path as the first CLI argument.
    binary = sys.argv[1] if len(sys.argv) > 1 else None
    results = run_all(amem_binary=binary, use_mock=True)

    # Exit with non-zero status if any protocol failed.
    all_passed = all(ok for _, ok, _ in results)
    sys.exit(0 if all_passed else 1)


if __name__ == "__main__":
    main()
