"""Phase 7B validation report generator."""

from __future__ import annotations

from datetime import datetime
from pathlib import Path

from validation_7b.harness import CrossProviderHarness


def generate_report(harness: CrossProviderHarness, output_path: str | None = None) -> str:
    """Generate the Phase 7B validation report."""

    summary = harness.summary()
    results = summary["results"]

    # Group results by category
    cross_provider = [
        r for r in results
        if "Transfer" in r.name or "Continuity" in r.name or "Accumulation" in r.name
    ]
    provider_switch = [
        r for r in results
        if "Switch" in r.name or "Alternating" in r.name or "Relay" in r.name
    ]
    integrity = [
        r for r in results
        if any(kw in r.name for kw in [
            "Integrity", "Fingerprint", "Fidelity", "Health", "Size", "Format",
        ])
    ]

    lines: list[str] = []
    lines.append("# AgenticMemory Phase 7B -- Cross-Provider Validation Report")
    lines.append(f"\n**Generated:** {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    lines.append(f"**Total Tests:** {summary['total']}")
    lines.append(f"**Passed:** {summary['passed']}")
    lines.append(f"**Failed:** {summary['failed']}")
    lines.append(f"**Skipped:** {summary['skipped']}")
    lines.append(f"**Pass Rate:** {summary['pass_rate']:.0%}")

    # Providers tested
    providers_seen: set[str] = set()
    for r in results:
        if not r.skipped:
            providers_seen.add(r.provider_a)
            providers_seen.add(r.provider_b)
    providers_seen.discard("N/A")
    providers_seen.discard("all")
    lines.append(f"**Providers Tested:** {', '.join(sorted(providers_seen))}")

    # Cross-Provider Tests
    lines.append("\n---\n")
    lines.append("## Cross-Provider Memory Transfer Tests\n")
    lines.append(
        "These tests verify that memories written by one LLM provider "
        "can be read and understood by a different provider.\n"
    )
    lines.append("| Test | Provider A -> B | Result | Time |")
    lines.append("|------|---------------|--------|------|")
    for r in cross_provider:
        status = "PASS" if r.passed else ("SKIP" if r.skipped else "FAIL")
        lines.append(
            f"| {r.name} | {r.provider_a} -> {r.provider_b} | {status} | {r.duration_seconds:.1f}s |"
        )

    # Provider Switch Tests
    lines.append("\n---\n")
    lines.append("## Provider Switch Tests\n")
    lines.append(
        "These tests simulate real-world provider switching: "
        "extended use with one provider, then switching to another.\n"
    )
    lines.append("| Test | Providers | Result | Time |")
    lines.append("|------|-----------|--------|------|")
    for r in provider_switch:
        status = "PASS" if r.passed else ("SKIP" if r.skipped else "FAIL")
        lines.append(
            f"| {r.name} | {r.provider_a} <-> {r.provider_b} | {status} | {r.duration_seconds:.1f}s |"
        )

    # Brain Integrity Tests
    lines.append("\n---\n")
    lines.append("## Brain File Integrity Tests\n")
    lines.append(
        "These tests verify the `.amem` file format is provider-agnostic "
        "and structurally sound.\n"
    )
    lines.append("| Test | Provider(s) | Result | Time |")
    lines.append("|------|------------|--------|------|")
    for r in integrity:
        status = "PASS" if r.passed else ("SKIP" if r.skipped else "FAIL")
        providers = r.provider_a if r.provider_a == r.provider_b else f"{r.provider_a} <-> {r.provider_b}"
        lines.append(f"| {r.name} | {providers} | {status} | {r.duration_seconds:.1f}s |")

    # Failures detail
    failures = [r for r in results if not r.passed and not r.skipped]
    if failures:
        lines.append("\n---\n")
        lines.append("## Failures\n")
        for r in failures:
            lines.append(f"### {r.name}")
            lines.append(f"- **Providers:** {r.provider_a} -> {r.provider_b}")
            lines.append(f"- **Error:** {r.error}")
            lines.append("")

    # Summary
    lines.append("\n---\n")
    lines.append("## Summary\n")

    if summary["failed"] == 0 and summary["passed"] > 0:
        lines.append(
            "**All tests passed.** The AgenticMemory `.amem` format is fully "
            "portable across LLM providers."
        )
        lines.append("")
        lines.append("Key findings:")
        lines.append(
            "- Facts, decisions, inferences, and skills transfer seamlessly between providers"
        )
        lines.append("- Corrections made by one provider are respected by all others")
        lines.append("- The binary format contains no provider-specific data")
        lines.append("- Memory persists correctly across provider switches at any point")
        lines.append("- File size grows linearly and predictably regardless of provider")
    elif summary["failed"] > 0:
        lines.append(
            f"**{summary['failed']} test(s) failed.** See Failures section above for details."
        )

    if summary["skipped"] > 0:
        lines.append(
            f"\n*{summary['skipped']} test(s) were skipped due to unavailable backends.*"
        )

    # Timing
    total_time = sum(r.duration_seconds for r in results if not r.skipped)
    lines.append(f"\n**Total validation time:** {total_time:.0f}s ({total_time / 60:.1f} minutes)")

    report_text = "\n".join(lines)

    # Print to terminal
    print(f"\n{'=' * 60}")
    print(report_text)
    print(f"{'=' * 60}")

    # Save to file
    if output_path is None:
        output_path = "validation_7b_report.md"
    Path(output_path).write_text(report_text)
    print(f"\nReport saved to: {output_path}")

    return report_text
