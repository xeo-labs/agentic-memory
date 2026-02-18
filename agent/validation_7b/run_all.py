"""Main entry point — runs all Phase 7B validation tests."""

from __future__ import annotations

import argparse
import sys

from validation_7b.harness import CrossProviderHarness
from validation_7b.cross_provider_tests import run_cross_provider_tests
from validation_7b.provider_switch_tests import run_provider_switch_tests
from validation_7b.brain_integrity import run_brain_integrity_tests
from validation_7b.report import generate_report


def main(report_path: str | None = None) -> bool:
    """Run all Phase 7B validation tests.

    Returns True if all non-skipped tests passed.
    """
    print("=" * 60)
    print("AgenticMemory Phase 7B — Cross-Provider Validation")
    print("=" * 60)

    harness = CrossProviderHarness()
    has_multiple = harness.setup()

    if has_multiple:
        # Run cross-provider tests for every available pair
        run_cross_provider_tests(harness)
        # Run provider switch tests
        run_provider_switch_tests(harness)

    # Brain integrity tests (always run, even single-provider)
    run_brain_integrity_tests(harness)

    # Generate report
    generate_report(harness, output_path=report_path)

    # Summary
    summary = harness.summary()
    print(f"\n{'=' * 60}")
    print(
        f"PHASE 7B RESULTS: {summary['passed']}/{summary['total']} passed, "
        f"{summary['skipped']} skipped, {summary['failed']} failed"
    )
    print(f"{'=' * 60}")

    return summary["failed"] == 0


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Run Phase 7B cross-provider validation")
    parser.add_argument(
        "--report-path",
        default=None,
        help="Output path for the report markdown file (default: validation_7b_report.md)",
    )
    args = parser.parse_args()

    success = main(report_path=args.report_path)
    sys.exit(0 if success else 1)
