"""Provider switch tests — simulate real-world provider switching patterns."""

from __future__ import annotations

import shutil
import time

from amem_agent.brain import Brain
from amem_agent.llm.base import LLMBackend
from validation_7b.harness import CrossProviderHarness, TestResult, _find_amem_binary
from validation_7b.helpers import (
    CrossProviderSession,
    assert_response_contains,
    assert_response_contains_any,
    run_with_retry,
)


# ------------------------------------------------------------------
# Test S1: Clean Switch After 10 Sessions
# ------------------------------------------------------------------

def test_clean_switch_10_sessions(backend_a: LLMBackend, backend_b: LLMBackend, brain_path: str):
    """10 sessions with Provider A building real context. Then Provider B takes over."""

    context_messages = [
        # Session 1: Personal intro
        ("My name is Raj Patel. I'm a data scientist living in Toronto.", None),
        # Session 2: Work context
        ("I work at Quantum Analytics. We build ML pipelines for financial firms.", None),
        # Session 3: Current project
        ("I'm building a fraud detection model using XGBoost. The dataset has 2M transactions.", None),
        # Session 4: Technical preference
        ("I prefer Jupyter notebooks for exploration but production code must be proper Python packages.", None),
        # Session 5: Team context
        (
            "My team is 4 people. Sarah handles data engineering, Mike does the frontend dashboards, "
            "and Lisa is our PM.",
            None,
        ),
        # Session 6: Technical decision
        (
            "We decided to use MLflow for experiment tracking instead of Weights & Biases "
            "because we need on-prem.",
            "What were we tracking experiments with?",
        ),
        # Session 7: Personal detail
        ("I'm getting married next October. Planning is stressful but exciting.", None),
        # Session 8: Correction
        (
            "Actually, our team just grew. We hired two new junior data scientists, Arun and Priya.",
            None,
        ),
        # Session 9: Skill learning
        (
            "When I ask about ML models, please compare trade-offs in a table format. That helps me think.",
            None,
        ),
        # Session 10: Project update
        (
            "The fraud model hit 94.2% precision and 91.7% recall on the test set. "
            "We're deploying next week.",
            None,
        ),
    ]

    for i, (message, followup) in enumerate(context_messages):
        sa = CrossProviderSession(brain_path, backend_a, session_id=i + 1, provider_name=backend_a.name())
        sa.send(message)
        if followup:
            sa.send(followup)
        sa.close()

    # === Phase 2: Provider B takes over ===
    sb = CrossProviderSession(brain_path, backend_b, session_id=11, provider_name=backend_b.name())

    # Test recall of core facts
    r1 = sb.send("What's my name and where do I work?")
    assert assert_response_contains(r1, ["Raj"]) or assert_response_contains(r1, ["Patel"]), \
        "Provider B forgot name after switch"
    assert assert_response_contains_any(r1, ["Quantum", "Analytics"]), \
        "Provider B forgot company after switch"

    # Test recall of project details
    r2 = sb.send("How is my fraud detection model performing?")
    assert assert_response_contains_any(r2, ["94", "precision", "recall", "fraud", "XGBoost"]), \
        "Provider B lost project metrics after switch"

    # Test recall of team
    r3 = sb.send("Who is on my team?")
    team_members = ["Sarah", "Mike", "Lisa", "Arun", "Priya"]
    found = sum(1 for name in team_members if name.lower() in r3.lower())
    assert found >= 3, \
        f"Provider B only recalled {found}/5 team members after switch"

    # Test recall of correction (team grew)
    assert assert_response_contains_any(r3, ["Arun", "Priya", "junior", "new", "grew"]), \
        "Provider B didn't pick up the team growth correction"

    # Test recall of preferences
    sb.send("Compare random forests vs gradient boosting for me.")

    sb.close()

    # Verify brain health
    brain = Brain(brain_path, amem_binary=_find_amem_binary())
    info = brain.info()
    assert info.session_count >= 11, f"Brain lost sessions during switch"
    assert info.node_count >= 15, f"Brain has too few nodes: {info.node_count}"


# ------------------------------------------------------------------
# Test S2: Alternating Providers Every 2 Sessions
# ------------------------------------------------------------------

def test_alternating_providers(backend_a: LLMBackend, backend_b: LLMBackend, brain_path: str):
    """Alternate between providers every 2 sessions for 10 total sessions."""

    messages_by_session = {
        1: "I'm starting a new project called Apollo — a distributed task queue in Rust.",
        2: "Apollo needs to support priority queues, retry logic, and dead letter queues.",
        3: "I chose Redis as the backing store for Apollo because of its speed and pub/sub support.",
        4: "I'm using Tokio for async runtime. The worker pool should scale to 1000 concurrent tasks.",
        5: "I added a web dashboard using Axum. It shows queue depth, failure rates, and worker status.",
        6: "Testing is tricky. I'm using testcontainers-rs to spin up Redis instances in tests.",
        7: "I hit a bug — tasks were being double-processed due to a race condition in the ack logic.",
        8: "Fixed the bug by switching from optimistic locking to Redis WATCH/MULTI transactions.",
        9: "Apollo v0.2 is ready. Benchmarks show 50,000 tasks/second throughput on a single node.",
        10: "I'm writing documentation now. Going to open source it next month.",
    }

    for session_num, message in messages_by_session.items():
        backend = backend_a if session_num % 2 == 1 else backend_b
        provider_name = backend.name()

        s = CrossProviderSession(brain_path, backend, session_id=session_num, provider_name=provider_name)
        s.send(message)
        s.close()

    # === Final verification session with Provider A ===
    s_final = CrossProviderSession(brain_path, backend_a, session_id=11, provider_name=backend_a.name())

    r = s_final.send(
        "Give me a full summary of the Apollo project — what it is, "
        "the tech stack, current status, and any issues we hit."
    )

    assert assert_response_contains_any(r, ["Apollo"]), "Lost project name"
    assert assert_response_contains_any(r, ["task queue", "distributed", "queue"]), \
        "Lost project description"
    assert assert_response_contains_any(r, ["Rust", "rust"]), "Lost language choice"
    assert assert_response_contains_any(r, ["Redis", "redis"]), "Lost backing store"

    detail_keywords = [
        "Tokio", "Axum", "race condition", "bug", "50000", "50,000",
        "throughput", "open source", "dashboard",
    ]
    details_found = sum(1 for kw in detail_keywords if kw.lower() in r.lower())
    assert details_found >= 3, \
        f"Only recalled {details_found} project details. Response: {r[:500]}"

    s_final.close()


# ------------------------------------------------------------------
# Test S3: Correction Across Provider Switch
# ------------------------------------------------------------------

def test_correction_across_switch(backend_a: LLMBackend, backend_b: LLMBackend, brain_path: str):
    """
    Provider A: establishes fact
    Provider B: corrects it
    Provider A: must use corrected version
    """

    # === Provider A establishes fact ===
    s1 = CrossProviderSession(brain_path, backend_a, session_id=1, provider_name=backend_a.name())
    s1.send("My favorite restaurant is Sakura Sushi on Oak Street.")
    s1.close()

    # === Provider B corrects it ===
    s2 = CrossProviderSession(brain_path, backend_b, session_id=2, provider_name=backend_b.name())
    s2.send("Sakura Sushi closed down last month. My new favorite restaurant is Ember Grill on Pine Avenue.")
    s2.close()

    # === Provider A must use corrected version ===
    s3 = CrossProviderSession(brain_path, backend_a, session_id=3, provider_name=backend_a.name())
    r = s3.send("What's my favorite restaurant?")

    assert assert_response_contains_any(r, ["Ember", "Grill"]), \
        f"Provider A didn't respect Provider B's correction. Got: {r}"

    s3.close()


# ------------------------------------------------------------------
# Test S4: Three-Provider Relay
# ------------------------------------------------------------------

def test_three_provider_relay(
    backend_a: LLMBackend,
    backend_b: LLMBackend,
    backend_c: LLMBackend,
    brain_path: str,
):
    """
    Provider A: personal facts
    Provider B: work facts
    Provider C: must recall both
    """

    # === Provider A ===
    s1 = CrossProviderSession(brain_path, backend_a, session_id=1, provider_name=backend_a.name())
    s1.send("I'm Alex Chen. I live in Seattle with my partner Jamie and our cat Pixel.")
    s1.close()

    # === Provider B ===
    s2 = CrossProviderSession(brain_path, backend_b, session_id=2, provider_name=backend_b.name())
    s2.send(
        "I just started a new role as VP of Engineering at CloudScale Inc. "
        "Managing a team of 40 engineers."
    )
    s2.close()

    # === Provider C: knows everything ===
    s3 = CrossProviderSession(brain_path, backend_c, session_id=3, provider_name=backend_c.name())
    r = s3.send("Tell me everything you know about me.")

    personal = assert_response_contains_any(r, ["Alex", "Seattle", "Jamie", "Pixel"])
    work = assert_response_contains_any(r, ["VP", "Engineering", "CloudScale", "40 engineer"])

    assert personal, "Provider C lost Provider A's personal facts"
    assert work, "Provider C lost Provider B's work facts"

    s3.close()


# ------------------------------------------------------------------
# Runner
# ------------------------------------------------------------------

def run_provider_switch_tests(harness: CrossProviderHarness):
    """Run all provider switch tests."""
    pairs = harness.detector.get_available_pairs()
    available = harness.detector.get_available_backends()

    print(f"\n{'=' * 60}")
    print("PROVIDER SWITCH TESTS")
    print(f"{'=' * 60}")

    two_provider_tests = [
        ("Clean Switch After 10 Sessions", test_clean_switch_10_sessions),
        ("Alternating Providers", test_alternating_providers),
        ("Correction Across Switch", test_correction_across_switch),
    ]

    for backend_a, backend_b in pairs:
        print(f"\n  {backend_a.name()} <-> {backend_b.name()}")

        for test_name, test_fn in two_provider_tests:
            tmpdir, brain_path = harness.create_test_brain()
            start = time.time()

            try:
                _fn = test_fn
                _ba = backend_a
                _bb = backend_b
                _bp = brain_path
                run_with_retry(lambda: _fn(_ba, _bb, _bp))
                duration = time.time() - start
                harness.record_result(TestResult(
                    name=test_name,
                    provider_a=backend_a.name(),
                    provider_b=backend_b.name(),
                    passed=True, skipped=False,
                    duration_seconds=duration,
                    details="PASS",
                ))
                print(f"    PASS {test_name} ({duration:.1f}s)")

            except Exception as e:
                duration = time.time() - start
                harness.record_result(TestResult(
                    name=test_name,
                    provider_a=backend_a.name(),
                    provider_b=backend_b.name(),
                    passed=False, skipped=False,
                    duration_seconds=duration,
                    details=f"FAIL: {e}",
                    error=str(e),
                ))
                print(f"    FAIL {test_name} ({duration:.1f}s) -- {e}")

            finally:
                shutil.rmtree(tmpdir, ignore_errors=True)

    # Three-provider relay (only if 3 backends available)
    if len(available) >= 3:
        print(
            f"\n  Three-Provider Relay: "
            f"{available[0].name()} -> {available[1].name()} -> {available[2].name()}"
        )
        tmpdir, brain_path = harness.create_test_brain()
        start = time.time()

        try:
            _a, _b, _c, _bp = available[0], available[1], available[2], brain_path
            run_with_retry(lambda: test_three_provider_relay(_a, _b, _c, _bp))
            duration = time.time() - start
            harness.record_result(TestResult(
                name="Three-Provider Relay",
                provider_a=f"{available[0].name()}+{available[1].name()}",
                provider_b=available[2].name(),
                passed=True, skipped=False,
                duration_seconds=duration,
                details="PASS",
            ))
            print(f"    PASS Three-Provider Relay ({duration:.1f}s)")

        except Exception as e:
            duration = time.time() - start
            harness.record_result(TestResult(
                name="Three-Provider Relay",
                provider_a=f"{available[0].name()}+{available[1].name()}",
                provider_b=available[2].name(),
                passed=False, skipped=False,
                duration_seconds=duration,
                details=f"FAIL: {e}",
                error=str(e),
            ))
            print(f"    FAIL Three-Provider Relay ({duration:.1f}s) -- {e}")

        finally:
            shutil.rmtree(tmpdir, ignore_errors=True)
    else:
        harness.record_result(TestResult(
            name="Three-Provider Relay",
            provider_a="N/A", provider_b="N/A",
            passed=False, skipped=True,
            duration_seconds=0,
            details=f"SKIPPED: Need 3 backends, only have {len(available)}",
        ))
        print("    SKIP Three-Provider Relay -- SKIPPED (need 3 backends)")
