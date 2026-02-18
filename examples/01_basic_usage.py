#!/usr/bin/env python3
"""
01_basic_usage.py — Simplest possible AgenticMemory example.

Demonstrates:
  - Creating a Brain instance
  - Adding facts and a decision
  - Linking events with causal edges
  - Querying stored facts and decisions
"""

from agentic_memory import Brain, EdgeType

def main():
    # Create an in-memory brain (uses default local storage)
    brain = Brain()

    session = "onboarding"

    # Store three facts the agent has learned
    f1 = brain.add_fact("The user prefers Python for backend work", session)
    f2 = brain.add_fact("The project deadline is March 15", session)
    f3 = brain.add_fact("The team uses PostgreSQL for persistence", session)

    # Record a decision derived from those facts
    d1 = brain.add_decision("Use FastAPI with SQLAlchemy for the backend", session)

    # Link the facts to the decision (facts caused the decision)
    brain.link(f1, d1, EdgeType.CAUSED_BY)
    brain.link(f2, d1, EdgeType.CAUSED_BY)
    brain.link(f3, d1, EdgeType.CAUSED_BY)

    # Query everything back
    print("=== Stored Facts ===")
    for fact in brain.facts(limit=10):
        print(f"  [{fact.id}] {fact.content}")

    print("\n=== Stored Decisions ===")
    for dec in brain.decisions(limit=10):
        print(f"  [{dec.id}] {dec.content}")

    # Show brain stats
    print(f"\n=== Brain Info ===\n  {brain.info()}")


if __name__ == "__main__":
    main()
