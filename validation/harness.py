"""Cross-provider test harness — manages test execution and results."""

from __future__ import annotations

import os
import subprocess
import tempfile
from dataclasses import dataclass, field
from pathlib import Path

from amem_agent.brain import Brain
from validation_7b.backend_detector import BackendDetector


# Auto-detect the amem binary location.
_AMEM_BINARY: str | None = None


def _find_amem_binary() -> str:
    """Locate the amem binary. Tries PATH first, then known build directories."""
    global _AMEM_BINARY
    if _AMEM_BINARY is not None:
        return _AMEM_BINARY

    # 1. Check PATH
    try:
        subprocess.run(["which", "amem"], capture_output=True, check=True, timeout=5)
        _AMEM_BINARY = "amem"
        return _AMEM_BINARY
    except (subprocess.CalledProcessError, FileNotFoundError):
        pass

    # 2. Check known build paths relative to amem-agent
    agent_dir = Path(__file__).resolve().parent.parent
    candidates = [
        agent_dir.parent / "target" / "release" / "amem",
        agent_dir.parent / "target" / "debug" / "amem",
    ]
    for candidate in candidates:
        if candidate.is_file() and os.access(str(candidate), os.X_OK):
            _AMEM_BINARY = str(candidate)
            return _AMEM_BINARY

    # 3. Fallback — let Brain raise a nice error
    _AMEM_BINARY = "amem"
    return _AMEM_BINARY


@dataclass
class TestResult:
    name: str                       # Test name
    provider_a: str                 # First provider used
    provider_b: str                 # Second provider used (or same for single-provider)
    passed: bool                    # Did it pass?
    skipped: bool                   # Was it skipped (missing backend)?
    duration_seconds: float         # How long it took
    details: str                    # Pass/fail details
    error: str | None = None        # Error message if failed


class CrossProviderHarness:
    """Manages cross-provider test execution."""

    def __init__(self):
        self.detector = BackendDetector()
        self.results: list[TestResult] = []
        self.amem_binary = _find_amem_binary()

    def setup(self) -> bool:
        """Detect backends and validate we can run tests.
        Returns True if at least 2 backends are available."""
        availability = self.detector.detect_all()

        print("Backend Detection:")
        for ba in availability:
            status = "Available" if ba.available else f"Unavailable ({ba.reason})"
            print(f"  {ba.name:12s} {status}")

        available = [ba for ba in availability if ba.available]
        if len(available) < 2:
            print(f"\n  Only {len(available)} backend(s) available. Need 2+ for cross-provider tests.")
            print("  Cross-provider tests will be skipped.")
            print("  Single-provider tests will still run.")
            return False

        pairs = self.detector.get_available_pairs()
        print(f"\n  {len(available)} backends available -> {len(pairs)} test pair(s)")
        for a, b in pairs:
            print(f"    {a.name()} <-> {b.name()}")

        return True

    def create_test_brain(self) -> tuple[str, str]:
        """Create a temporary directory with a fresh brain file.
        Returns (temp_dir_path, brain_file_path).
        Caller is responsible for cleanup."""
        tmpdir = tempfile.mkdtemp(prefix="amem_7b_")
        brain_path = str(Path(tmpdir) / "test_brain.amem")
        brain = Brain(brain_path, amem_binary=self.amem_binary)
        brain.ensure_exists()
        return tmpdir, brain_path

    def record_result(self, result: TestResult):
        """Record a test result."""
        self.results.append(result)

    def summary(self) -> dict:
        """Generate summary of all test results."""
        passed = sum(1 for r in self.results if r.passed)
        failed = sum(1 for r in self.results if not r.passed and not r.skipped)
        skipped = sum(1 for r in self.results if r.skipped)
        return {
            "total": len(self.results),
            "passed": passed,
            "failed": failed,
            "skipped": skipped,
            "pass_rate": passed / max(len(self.results) - skipped, 1),
            "results": self.results,
        }
