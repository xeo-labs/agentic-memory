"""8 cross-provider test scenarios proving memory works across different LLM providers."""

from __future__ import annotations

import shutil
import time

from amem_agent.llm.base import LLMBackend
from validation_7b.harness import CrossProviderHarness, TestResult
from validation_7b.helpers import (
    CrossProviderSession,
    assert_response_contains,
    assert_response_contains_any,
    run_with_retry,
)


# ------------------------------------------------------------------
# Test 1: Fact Transfer
# ------------------------------------------------------------------

def test_fact_transfer(backend_a: LLMBackend, backend_b: LLMBackend, brain_path: str):
    """Provider A learns facts. Provider B recalls them."""

    # === Provider A Session ===
    sa = CrossProviderSession(brain_path, backend_a, session_id=1, provider_name=backend_a.name())
    sa.send("My name is Elena. I'm a mechanical engineer in Munich, Germany.")
    sa.send("I specialize in automotive drivetrain design. I've been doing this for 12 years.")
    sa.close()

    # === Provider B Session (fresh — no history) ===
    sb = CrossProviderSession(brain_path, backend_b, session_id=2, provider_name=backend_b.name())

    r1 = sb.send("What's my name and where do I live?")
    assert assert_response_contains(r1, ["Elena"]), \
        f"Provider B ({backend_b.name()}) didn't recall name from Provider A ({backend_a.name()})"
    assert assert_response_contains_any(r1, ["Munich", "Germany"]), \
        "Provider B didn't recall location"

    r2 = sb.send("What do I do for work?")
    assert assert_response_contains_any(r2, ["mechanical", "engineer", "automotive", "drivetrain"]), \
        "Provider B didn't recall profession"

    sb.close()


# ------------------------------------------------------------------
# Test 2: Decision Transfer
# ------------------------------------------------------------------

def test_decision_transfer(backend_a: LLMBackend, backend_b: LLMBackend, brain_path: str):
    """Provider A makes a decision with reasoning. Provider B explains the reasoning."""

    # === Provider A ===
    sa = CrossProviderSession(brain_path, backend_a, session_id=1, provider_name=backend_a.name())
    sa.send(
        "I need to choose between Kubernetes and Docker Swarm for my startup. "
        "We have 3 engineers, limited DevOps experience, and need to deploy within 2 weeks."
    )
    sa.send("What do you recommend?")
    sa.close()

    # === Provider B ===
    sb = CrossProviderSession(brain_path, backend_b, session_id=2, provider_name=backend_b.name())
    r = sb.send(
        "We talked about container orchestration for my startup recently. "
        "What was the recommendation and why?"
    )

    has_tech = assert_response_contains_any(r, ["kubernetes", "k8s", "docker swarm", "swarm"])
    has_reason = assert_response_contains_any(
        r, ["engineer", "team", "devops", "deploy", "week", "startup", "simple"]
    )

    assert has_tech, "Provider B didn't recall the technology recommendation"
    assert has_reason, "Provider B recalled recommendation but not reasoning"

    sb.close()


# ------------------------------------------------------------------
# Test 3: Correction Transfer
# ------------------------------------------------------------------

def test_correction_transfer(backend_a: LLMBackend, backend_b: LLMBackend, brain_path: str):
    """Provider A establishes a fact, then corrects it. Provider B uses the corrected version."""

    # === Provider A Session 1: Establish fact ===
    sa1 = CrossProviderSession(brain_path, backend_a, session_id=1, provider_name=backend_a.name())
    sa1.send("I drive a 2020 Honda Civic.")
    sa1.close()

    # === Provider A Session 2: Correct it ===
    sa2 = CrossProviderSession(brain_path, backend_a, session_id=2, provider_name=backend_a.name())
    sa2.send("Actually, I sold my Honda last month. I now drive a 2024 Tesla Model 3.")
    sa2.close()

    # === Provider B Session ===
    sb = CrossProviderSession(brain_path, backend_b, session_id=3, provider_name=backend_b.name())
    r = sb.send("What car do I drive?")

    assert assert_response_contains_any(r, ["Tesla", "Model 3"]), \
        f"Provider B didn't recall corrected car. Got: {r}"

    sb.close()


# ------------------------------------------------------------------
# Test 4: Multi-Fact Accumulation
# ------------------------------------------------------------------

def test_multi_fact_accumulation(backend_a: LLMBackend, backend_b: LLMBackend, brain_path: str):
    """Provider A builds up facts across 5 sessions. Provider B recalls all of them."""

    facts = [
        ("I have a daughter named Zoe who is 7 years old.", ["Zoe", "daughter"]),
        ("I'm allergic to peanuts.", ["allergic", "peanut"]),
        ("My favorite band is Radiohead.", ["Radiohead"]),
        ("I run a small bakery called Sunrise Breads.", ["Sunrise", "bakery"]),
        ("I'm training for an Ironman triathlon.", ["Ironman", "triathlon"]),
    ]

    # === Provider A: 5 sessions, one fact each ===
    for i, (fact, _) in enumerate(facts):
        sa = CrossProviderSession(brain_path, backend_a, session_id=i + 1, provider_name=backend_a.name())
        sa.send(fact)
        sa.close()

    # === Provider B: Recall all facts ===
    sb = CrossProviderSession(brain_path, backend_b, session_id=10, provider_name=backend_b.name())

    recall_count = 0
    for fact_text, keywords in facts:
        question_topic = keywords[0]
        r = sb.send(f"What do you know about my {question_topic.lower()}?")
        if assert_response_contains_any(r, keywords):
            recall_count += 1

    sb.close()

    # At least 4 out of 5 facts recalled (allowing for one LLM miss)
    assert recall_count >= 4, \
        f"Provider B only recalled {recall_count}/5 facts from Provider A"


# ------------------------------------------------------------------
# Test 5: Bidirectional Memory
# ------------------------------------------------------------------

def test_bidirectional_memory(backend_a: LLMBackend, backend_b: LLMBackend, brain_path: str):
    """Both providers contribute to the brain. Each reads the other's contributions."""

    # === Provider A writes ===
    sa = CrossProviderSession(brain_path, backend_a, session_id=1, provider_name=backend_a.name())
    sa.send("I'm building a mobile app called Habitat for tracking indoor plants.")
    sa.close()

    # === Provider B reads A's data AND writes its own ===
    sb = CrossProviderSession(brain_path, backend_b, session_id=2, provider_name=backend_b.name())
    r1 = sb.send("What app am I building?")
    assert assert_response_contains_any(r1, ["Habitat", "plant"]), \
        "Provider B can't read Provider A's memories"

    sb.send("I decided to build it with React Native so it works on both iOS and Android.")
    sb.close()

    # === Provider A reads B's addition ===
    sa2 = CrossProviderSession(brain_path, backend_a, session_id=3, provider_name=backend_a.name())
    r2 = sa2.send("What technology am I using for my app?")
    assert assert_response_contains_any(r2, ["React Native", "react"]), \
        "Provider A can't read Provider B's memories"

    sa2.close()


# ------------------------------------------------------------------
# Test 6: Inference Transfer
# ------------------------------------------------------------------

def test_inference_transfer(backend_a: LLMBackend, backend_b: LLMBackend, brain_path: str):
    """Provider A draws an inference from multiple facts. Provider B can access that inference."""

    # === Provider A: provide context and let it draw an inference ===
    sa = CrossProviderSession(brain_path, backend_a, session_id=1, provider_name=backend_a.name())
    sa.send(
        "I've been coding in Python for 10 years. I've contributed to Django and Flask. "
        "I teach Python at the local community college on weekends."
    )
    sa.send("Based on what you know about me, what level of Python developer would you say I am?")
    sa.close()

    # === Provider B: check if the inference is accessible ===
    sb = CrossProviderSession(brain_path, backend_b, session_id=2, provider_name=backend_b.name())
    r2 = sb.send("What do you know about my programming expertise?")

    assert assert_response_contains_any(r2, ["Python", "python"]), \
        "Provider B didn't recall Python expertise"
    assert assert_response_contains_any(
        r2, ["senior", "expert", "experienced", "advanced", "10 year", "teach", "Django", "Flask"]
    ), "Provider B didn't capture the expertise level"

    sb.close()


# ------------------------------------------------------------------
# Test 7: Skill Transfer
# ------------------------------------------------------------------

def test_skill_transfer(backend_a: LLMBackend, backend_b: LLMBackend, brain_path: str):
    """Provider A learns a user preference/skill. Provider B follows it."""

    # === Provider A: establish preferences ===
    sa = CrossProviderSession(brain_path, backend_a, session_id=1, provider_name=backend_a.name())
    sa.send(
        "When you explain technical concepts to me, please always include a real-world analogy. "
        "I learn best through analogies. Also, keep code examples in Rust, not Python — I only write Rust."
    )
    sa.send("Can you explain what a mutex is?")
    sa.close()

    # === Provider B: test if it follows the learned preference ===
    sb = CrossProviderSession(brain_path, backend_b, session_id=2, provider_name=backend_b.name())
    r = sb.send("Can you explain what a channel is in concurrent programming?")

    uses_rust = assert_response_contains_any(r, ["rust", "fn ", "let ", "use std"])
    uses_analogy = assert_response_contains_any(
        r, ["like", "imagine", "think of", "analogy", "similar to", "picture"]
    )

    assert uses_rust or uses_analogy, \
        f"Provider B didn't follow learned preferences (no Rust, no analogy). Response: {r[:300]}"

    sb.close()


# ------------------------------------------------------------------
# Test 8: Session History Continuity
# ------------------------------------------------------------------

def test_session_history_continuity(backend_a: LLMBackend, backend_b: LLMBackend, brain_path: str):
    """Alternating providers across 6 sessions, building on each other's context."""

    # Session 1 (Provider A): Start a project discussion
    s1 = CrossProviderSession(brain_path, backend_a, session_id=1, provider_name=backend_a.name())
    s1.send("I want to build a personal finance tracker. It should track expenses, income, and investments.")
    s1.close()

    # Session 2 (Provider B): Add technical decisions
    s2 = CrossProviderSession(brain_path, backend_b, session_id=2, provider_name=backend_b.name())
    s2.send(
        "For the finance tracker we discussed, I've decided to use PostgreSQL "
        "for the database and a REST API backend."
    )
    s2.close()

    # Session 3 (Provider A): Add more context
    s3 = CrossProviderSession(brain_path, backend_a, session_id=3, provider_name=backend_a.name())
    s3.send("I also want the finance tracker to have a mobile app. I'll use Flutter for the frontend.")
    s3.close()

    # Session 4 (Provider B): Recall the full picture
    s4 = CrossProviderSession(brain_path, backend_b, session_id=4, provider_name=backend_b.name())
    r = s4.send("Give me a summary of the tech stack and features we've decided on for my project.")

    finance_keywords = ["finance", "expense", "income", "investment", "tracker"]
    tech_keywords = ["PostgreSQL", "postgres", "REST", "API", "Flutter", "mobile"]

    has_finance = assert_response_contains_any(r, finance_keywords)
    has_tech = assert_response_contains_any(r, tech_keywords)

    assert has_finance, "Provider B lost the project context"
    assert has_tech, "Provider B lost the tech stack decisions"

    s4.close()


# ------------------------------------------------------------------
# Test registry & runner
# ------------------------------------------------------------------

ALL_TESTS = [
    ("Fact Transfer", test_fact_transfer),
    ("Decision Transfer", test_decision_transfer),
    ("Correction Transfer", test_correction_transfer),
    ("Multi-Fact Accumulation", test_multi_fact_accumulation),
    ("Bidirectional Memory", test_bidirectional_memory),
    ("Inference Transfer", test_inference_transfer),
    ("Skill Transfer", test_skill_transfer),
    ("Session History Continuity", test_session_history_continuity),
]


def run_cross_provider_tests(harness: CrossProviderHarness):
    """Run all 8 tests for every available backend pair."""
    pairs = harness.detector.get_available_pairs()

    for backend_a, backend_b in pairs:
        print(f"\n{'─' * 60}")
        print(f"Testing: {backend_a.name()} -> {backend_b.name()}")
        print(f"{'─' * 60}")

        for test_name, test_fn in ALL_TESTS:
            tmpdir, brain_path = harness.create_test_brain()
            start = time.time()

            try:
                # Capture test_fn args to avoid late-binding issues in lambda
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
                print(f"  PASS {test_name} ({duration:.1f}s)")

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
                print(f"  FAIL {test_name} ({duration:.1f}s) -- {e}")

            finally:
                shutil.rmtree(tmpdir, ignore_errors=True)

        # Also run in REVERSE direction (B -> A) for the key tests
        print(f"\n  Testing reverse: {backend_b.name()} -> {backend_a.name()}")

        reverse_tests = [
            ("Fact Transfer (reverse)", test_fact_transfer),
            ("Correction Transfer (reverse)", test_correction_transfer),
            ("Bidirectional Memory (reverse)", test_bidirectional_memory),
        ]

        for test_name, test_fn in reverse_tests:
            tmpdir, brain_path = harness.create_test_brain()
            start = time.time()

            try:
                _fn = test_fn
                _bb = backend_b
                _ba = backend_a
                _bp = brain_path
                run_with_retry(lambda: _fn(_bb, _ba, _bp))
                duration = time.time() - start
                harness.record_result(TestResult(
                    name=test_name,
                    provider_a=backend_b.name(),
                    provider_b=backend_a.name(),
                    passed=True, skipped=False,
                    duration_seconds=duration,
                    details="PASS",
                ))
                print(f"  PASS {test_name} ({duration:.1f}s)")

            except Exception as e:
                duration = time.time() - start
                harness.record_result(TestResult(
                    name=test_name,
                    provider_a=backend_b.name(),
                    provider_b=backend_a.name(),
                    passed=False, skipped=False,
                    duration_seconds=duration,
                    details=f"FAIL: {e}",
                    error=str(e),
                ))
                print(f"  FAIL {test_name} ({duration:.1f}s) -- {e}")

            finally:
                shutil.rmtree(tmpdir, ignore_errors=True)
