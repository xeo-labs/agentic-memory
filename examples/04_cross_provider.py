#!/usr/bin/env python3
"""
04_cross_provider.py — Same brain, different LLM providers.

Demonstrates:
  - A single Brain instance shared across Anthropic and OpenAI providers
  - Knowledge added through one provider is immediately visible to the other
  - Graceful fallback when an API key is missing

Requires at least one of:
  - ANTHROPIC_API_KEY environment variable
  - OPENAI_API_KEY environment variable
"""

import os
import sys

from agentic_memory import Brain, MemoryAgent, EdgeType
from agentic_memory import AnthropicProvider, OpenAIProvider

def make_provider(name):
    """Try to create a provider; return None if the API key is missing."""
    try:
        if name == "anthropic":
            return AnthropicProvider()
        elif name == "openai":
            return OpenAIProvider()
    except Exception as exc:
        print(f"  [skip] {name} provider unavailable: {exc}")
        return None


def main():
    brain = Brain()

    # ── Attempt to create both providers ─────────────────────────
    anthropic = make_provider("anthropic")
    openai = make_provider("openai")

    if not anthropic and not openai:
        print("Error: Set ANTHROPIC_API_KEY or OPENAI_API_KEY to run this example.")
        sys.exit(1)

    # ── Phase 1: Learn facts (prefer Anthropic, fall back to OpenAI)
    provider_a = anthropic or openai
    provider_a_name = "Anthropic" if anthropic else "OpenAI"

    print(f"=== Phase 1: Learning facts via {provider_a_name} ===")
    agent_a = MemoryAgent(brain, provider_a)

    r1 = agent_a.chat("The company sells B2B SaaS for logistics.", "session-1")
    print(f"  Agent: {r1}")

    r2 = agent_a.chat("Annual revenue is $4.2M with 60% margins.", "session-2")
    print(f"  Agent: {r2}")

    # Show what the brain recorded
    print(f"\n  Brain now has: {brain.info()}")

    # ── Phase 2: Query + extend with the other provider ──────────
    provider_b = openai if anthropic else anthropic
    provider_b_name = "OpenAI" if anthropic else "Anthropic"

    if provider_b:
        print(f"\n=== Phase 2: Querying + extending via {provider_b_name} ===")
        agent_b = MemoryAgent(brain, provider_b)

        r3 = agent_b.chat(
            "What do you know about the company so far?", "session-3"
        )
        print(f"  Agent: {r3}")

        r4 = agent_b.chat(
            "The company just raised a Series B of $25M.", "session-4"
        )
        print(f"  Agent: {r4}")
    else:
        print(f"\n  [skip] Only one provider available; cross-provider demo skipped.")

    # ── Final state ──────────────────────────────────────────────
    print(f"\n=== Final Brain State ===")
    print(f"  {brain.info()}")

    print("\n=== All Facts ===")
    for fact in brain.facts(limit=20):
        print(f"  [{fact.id}] {fact.content}")

    print("\n=== All Decisions ===")
    for dec in brain.decisions(limit=20):
        print(f"  [{dec.id}] {dec.content}")


if __name__ == "__main__":
    main()
