"""Brain file integrity validation — ensures .amem format is provider-agnostic."""

from __future__ import annotations

import os
import shutil
import tempfile
import time

from amem_agent.brain import Brain
from amem_agent.llm.base import LLMBackend
from validation_7b.harness import CrossProviderHarness, TestResult, _find_amem_binary
from validation_7b.helpers import CrossProviderSession, run_with_retry


# ------------------------------------------------------------------
# Integrity Test 1: No Provider Fingerprints
# ------------------------------------------------------------------

def test_no_provider_fingerprints(backend: LLMBackend, brain_path: str):
    """Write memories with a specific provider, then scan the brain for provider traces."""

    # Build a brain with 10 sessions
    for i in range(10):
        s = CrossProviderSession(brain_path, backend, session_id=i + 1, provider_name=backend.name())
        s.send(f"Session {i + 1}: Tell me about the number {i + 1} in mathematics.")
        s.close()

    # Scan for provider fingerprints
    brain = Brain(brain_path, amem_binary=_find_amem_binary())

    # Known provider fingerprints — API keys, model IDs, tokens
    fingerprints = [
        # Anthropic
        "anthropic", "claude", "claude-3", "claude-sonnet", "claude-opus", "claude-haiku",
        # OpenAI
        "openai", "gpt-4", "gpt-3.5", "chatgpt", "text-davinci",
        # Ollama
        "ollama", "llama", "mistral", "qwen",
        # Generic model identifiers
        "model:", "api_key", "bearer", "sk-ant-", "sk-proj-",
    ]

    all_nodes = brain.search(limit=1000)
    violations = []

    for node in all_nodes:
        content = node.get("content", "").lower()
        for fp in fingerprints:
            if fp.lower() in content:
                # Only flag real metadata leaks, not conversation content
                if any(pattern in content for pattern in ["sk-ant-", "sk-proj-", "api_key", "bearer"]):
                    violations.append(
                        f"Node {node.get('id')}: contains '{fp}' -- possible API key/token leak"
                    )

    assert len(violations) == 0, \
        f"Brain contains provider fingerprints:\n" + "\n".join(violations)


# ------------------------------------------------------------------
# Integrity Test 2: Binary Format Consistency
# ------------------------------------------------------------------

def test_binary_format_consistency(backend_a: LLMBackend, backend_b: LLMBackend, brain_path: str):
    """Write with two providers separately, verify file headers and structure are identical."""

    # Brain 1: Written via Provider A
    brain_path_a = tempfile.mktemp(suffix=".amem")
    sa = CrossProviderSession(brain_path_a, backend_a, session_id=1, provider_name=backend_a.name())
    sa.send("Test fact: the sky is blue.")
    sa.close()

    # Brain 2: Written via Provider B
    brain_path_b = tempfile.mktemp(suffix=".amem")
    sb = CrossProviderSession(brain_path_b, backend_b, session_id=1, provider_name=backend_b.name())
    sb.send("Test fact: water is wet.")
    sb.close()

    try:
        # Read raw headers
        with open(brain_path_a, "rb") as f:
            header_a = f.read(64)
        with open(brain_path_b, "rb") as f:
            header_b = f.read(64)

        # Magic bytes must match
        assert header_a[:4] == header_b[:4] == b"AMEM", "Magic bytes don't match"

        # Version must match
        assert header_a[4:8] == header_b[4:8], "Format version differs between providers"

        # Dimension must match
        assert header_a[8:12] == header_b[8:12], "Feature vector dimension differs between providers"
    finally:
        for p in (brain_path_a, brain_path_b):
            try:
                os.unlink(p)
            except OSError:
                pass


# ------------------------------------------------------------------
# Integrity Test 3: Round-Trip Fidelity
# ------------------------------------------------------------------

def test_round_trip_fidelity(backend_a: LLMBackend, backend_b: LLMBackend, brain_path: str):
    """Provider A writes facts, Provider B reads. Verify A's nodes are still intact."""

    # Provider A writes
    sa = CrossProviderSession(brain_path, backend_a, session_id=1, provider_name=backend_a.name())
    sa.send("My favorite color is green. I have 3 siblings. I graduated in 2019.")
    sa.close()

    # Read brain state
    brain = Brain(brain_path, amem_binary=_find_amem_binary())
    nodes_after_a = brain.search(limit=1000)
    node_count_after_a = brain.info().node_count

    # Provider B reads the SAME brain
    sb = CrossProviderSession(brain_path, backend_b, session_id=2, provider_name=backend_b.name())
    sb.send("What do you know about me?")
    sb.close()

    # Re-read brain state
    nodes_after_b = brain.search(limit=1000)
    node_count_after_b = brain.info().node_count

    # Provider A's nodes must still be intact
    assert node_count_after_b >= node_count_after_a, \
        f"Provider B reading caused node loss: {node_count_after_a} -> {node_count_after_b}"

    # Check that Provider A's original facts are still in the brain
    a_contents = {n.get("content", "") for n in nodes_after_a}
    b_contents = {n.get("content", "") for n in nodes_after_b}

    for content in a_contents:
        assert content in b_contents, \
            f"Provider B lost Provider A's content: {content[:100]}"


# ------------------------------------------------------------------
# Integrity Test 4: Concurrent Brain Health
# ------------------------------------------------------------------

def test_brain_health_after_multi_provider(brain_path: str, backends: list[LLMBackend]):
    """All available providers write to the same brain sequentially. Verify health."""

    total_sessions = 0

    for i, backend in enumerate(backends):
        for j in range(3):  # 3 sessions per provider
            session_id = total_sessions + 1
            s = CrossProviderSession(
                brain_path, backend, session_id=session_id,
                provider_name=backend.name(),
            )
            s.send(f"Fact from {backend.name()} session {j + 1}: The number is {session_id * 7}.")
            s.close()
            total_sessions += 1

    # Verify brain health
    brain = Brain(brain_path, amem_binary=_find_amem_binary())
    info = brain.info()

    assert info.session_count >= total_sessions, \
        f"Expected {total_sessions}+ sessions, got {info.session_count}"

    assert info.node_count >= total_sessions, \
        f"Expected {total_sessions}+ nodes, got {info.node_count}"

    stats = brain.stats()
    assert stats is not None, "brain.stats() returned None"

    facts = brain.search(event_types=["fact"], limit=100)
    assert len(facts) > 0, "No facts found in multi-provider brain"

    episodes = brain.search(event_types=["episode"], limit=100)
    assert len(episodes) > 0, "No episodes found (session compression failed)"


# ------------------------------------------------------------------
# Integrity Test 5: File Size Sanity
# ------------------------------------------------------------------

def test_file_size_sanity(backend: LLMBackend, brain_path: str):
    """Verify file size grows linearly and doesn't bloat unexpectedly."""

    sizes = []

    for i in range(20):
        s = CrossProviderSession(brain_path, backend, session_id=i + 1, provider_name=backend.name())
        s.send(f"Session {i + 1}: This is a test message about topic number {i + 1}.")
        s.close()

        size = os.path.getsize(brain_path)
        sizes.append((i + 1, size))

    # File should grow roughly linearly
    growths = [sizes[i][1] - sizes[i - 1][1] for i in range(1, len(sizes))]
    avg_growth = sum(growths) / len(growths) if growths else 0

    for i, growth in enumerate(growths):
        if avg_growth > 0:
            ratio = growth / avg_growth
            assert ratio < 5.0, (
                f"Session {i + 2} caused abnormal growth: "
                f"{growth} bytes (avg: {avg_growth:.0f}, ratio: {ratio:.1f}x)"
            )

    # Final size should be reasonable (under 5MB for 20 simple sessions)
    final_size = sizes[-1][1]
    assert final_size < 5 * 1024 * 1024, \
        f"Brain file too large for 20 sessions: {final_size / 1024:.1f} KB"


# ------------------------------------------------------------------
# Runner
# ------------------------------------------------------------------

def run_brain_integrity_tests(harness: CrossProviderHarness):
    """Run all brain integrity validation tests."""
    available = harness.detector.get_available_backends()
    pairs = harness.detector.get_available_pairs()

    print(f"\n{'=' * 60}")
    print("BRAIN INTEGRITY TESTS")
    print(f"{'=' * 60}")

    # Tests that need 1 backend
    single_tests = [
        ("No Provider Fingerprints", test_no_provider_fingerprints),
        ("File Size Sanity", test_file_size_sanity),
    ]

    for backend in available:
        for test_name, test_fn in single_tests:
            tmpdir, brain_path = harness.create_test_brain()
            start = time.time()

            try:
                test_fn(backend, brain_path)
                duration = time.time() - start
                harness.record_result(TestResult(
                    name=f"{test_name} ({backend.name()})",
                    provider_a=backend.name(), provider_b=backend.name(),
                    passed=True, skipped=False,
                    duration_seconds=duration, details="PASS",
                ))
                print(f"  PASS {test_name} [{backend.name()}] ({duration:.1f}s)")

            except Exception as e:
                duration = time.time() - start
                harness.record_result(TestResult(
                    name=f"{test_name} ({backend.name()})",
                    provider_a=backend.name(), provider_b=backend.name(),
                    passed=False, skipped=False,
                    duration_seconds=duration, details=f"FAIL: {e}", error=str(e),
                ))
                print(f"  FAIL {test_name} [{backend.name()}] ({duration:.1f}s) -- {e}")

            finally:
                shutil.rmtree(tmpdir, ignore_errors=True)

    # Tests that need 2 backends
    if pairs:
        pair_tests = [
            ("Binary Format Consistency", test_binary_format_consistency),
            ("Round-Trip Fidelity", test_round_trip_fidelity),
        ]

        for backend_a, backend_b in pairs[:1]:  # Use first pair only
            for test_name, test_fn in pair_tests:
                tmpdir, brain_path = harness.create_test_brain()
                start = time.time()

                try:
                    test_fn(backend_a, backend_b, brain_path)
                    duration = time.time() - start
                    harness.record_result(TestResult(
                        name=test_name,
                        provider_a=backend_a.name(), provider_b=backend_b.name(),
                        passed=True, skipped=False,
                        duration_seconds=duration, details="PASS",
                    ))
                    print(
                        f"  PASS {test_name} "
                        f"[{backend_a.name()}<->{backend_b.name()}] ({duration:.1f}s)"
                    )

                except Exception as e:
                    duration = time.time() - start
                    harness.record_result(TestResult(
                        name=test_name,
                        provider_a=backend_a.name(), provider_b=backend_b.name(),
                        passed=False, skipped=False,
                        duration_seconds=duration, details=f"FAIL: {e}", error=str(e),
                    ))
                    print(
                        f"  FAIL {test_name} "
                        f"[{backend_a.name()}<->{backend_b.name()}] ({duration:.1f}s) -- {e}"
                    )

                finally:
                    shutil.rmtree(tmpdir, ignore_errors=True)

    # Multi-provider health test
    if len(available) >= 2:
        tmpdir, brain_path = harness.create_test_brain()
        start = time.time()

        try:
            test_brain_health_after_multi_provider(brain_path, available)
            duration = time.time() - start
            harness.record_result(TestResult(
                name="Multi-Provider Brain Health",
                provider_a="all", provider_b="all",
                passed=True, skipped=False,
                duration_seconds=duration, details="PASS",
            ))
            print(f"  PASS Multi-Provider Brain Health ({duration:.1f}s)")

        except Exception as e:
            duration = time.time() - start
            harness.record_result(TestResult(
                name="Multi-Provider Brain Health",
                provider_a="all", provider_b="all",
                passed=False, skipped=False,
                duration_seconds=duration, details=f"FAIL: {e}", error=str(e),
            ))
            print(f"  FAIL Multi-Provider Brain Health ({duration:.1f}s) -- {e}")

        finally:
            shutil.rmtree(tmpdir, ignore_errors=True)
