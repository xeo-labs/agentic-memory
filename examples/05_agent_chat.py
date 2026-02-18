#!/usr/bin/env python3
"""
05_agent_chat.py — Full conversational agent with persistent memory.

Demonstrates:
  - Creating a MemoryAgent backed by a Brain and an LLM provider
  - Running multiple chat sessions where the agent remembers prior context
  - The agent automatically stores facts, decisions, and inferences in the brain

Requires:
  - ANTHROPIC_API_KEY environment variable
"""

import os
import sys

from agentic_memory import Brain, MemoryAgent, AnthropicProvider

def main():
    # ── Check for API key ────────────────────────────────────────
    if not os.environ.get("ANTHROPIC_API_KEY"):
        print("Error: Set ANTHROPIC_API_KEY to run this example.")
        print("  export ANTHROPIC_API_KEY='sk-ant-...'")
        sys.exit(1)

    brain = Brain()
    provider = AnthropicProvider()
    agent = MemoryAgent(brain, provider)

    # ── Session 1: Introduce the project ─────────────────────────
    print("=== Session 1: Project Introduction ===\n")

    response = agent.chat(
        "I'm building a recipe recommendation app. The target audience is "
        "busy parents who want healthy meals that take under 30 minutes.",
        session="kickoff",
    )
    print(f"  Agent: {response}\n")

    response = agent.chat(
        "We'll use React Native for the mobile app and Python with FastAPI "
        "for the backend. Data comes from a PostgreSQL database.",
        session="kickoff",
    )
    print(f"  Agent: {response}\n")

    # ── Session 2: Discuss constraints ───────────────────────────
    print("=== Session 2: Constraints & Requirements ===\n")

    response = agent.chat(
        "Our budget is $5,000 for the MVP. We need to launch in 8 weeks. "
        "The team is just me and one designer.",
        session="planning",
    )
    print(f"  Agent: {response}\n")

    # ── Session 3: Ask the agent to recall everything ────────────
    print("=== Session 3: Memory Recall ===\n")

    response = agent.chat(
        "Can you summarize everything you know about my project so far? "
        "Include the tech stack, audience, constraints, and timeline.",
        session="review",
    )
    print(f"  Agent: {response}\n")

    # ── Show what the brain accumulated ──────────────────────────
    print("=== Brain Contents ===\n")
    print(f"  {brain.info()}\n")

    print("  Facts:")
    for fact in brain.facts(limit=20):
        print(f"    - {fact.content}")

    print("\n  Decisions:")
    for dec in brain.decisions(limit=20):
        print(f"    - {dec.content}")


if __name__ == "__main__":
    main()
