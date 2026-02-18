#!/usr/bin/env python3
"""
03_reasoning_chains.py — Build and traverse a decision tree.

Demonstrates:
  - Constructing a multi-level reasoning chain with CAUSED_BY and SUPPORTS edges
  - Using traverse() to walk the graph from any starting node
  - Pretty-printing the full reasoning chain
"""

from agentic_memory import Brain, EdgeType

def print_chain(label, nodes, indent=0):
    """Helper to pretty-print a chain of events."""
    prefix = "  " * indent
    print(f"{prefix}{label}")
    for node in nodes:
        print(f"{prefix}  [{node.id}] ({node.event_type}) {node.content}")


def main():
    brain = Brain()
    session = "planning"

    # ── Layer 1: Three ground-truth facts ────────────────────────
    f1 = brain.add_fact("User base is growing 20% month-over-month", session)
    f2 = brain.add_fact("Current server costs are $2,400/month", session)
    f3 = brain.add_fact("P95 latency has increased to 800ms", session)

    # ── Layer 2: A tactical decision derived from the facts ──────
    d1 = brain.add_decision(
        "Migrate from monolith to microservices architecture", session
    )

    # Facts caused this decision
    brain.link(f1, d1, EdgeType.CAUSED_BY)
    brain.link(f2, d1, EdgeType.CAUSED_BY)
    brain.link(f3, d1, EdgeType.CAUSED_BY)

    # ── Layer 3: A strategic decision that the tactical one supports
    d2 = brain.add_decision(
        "Adopt Kubernetes for container orchestration", session,
        confidence=0.85,
    )

    # The microservices decision supports the Kubernetes decision
    brain.link(d1, d2, EdgeType.SUPPORTS)

    # Add an inference that ties everything together
    inf = brain.add_inference(
        "Microservices + Kubernetes should reduce P95 latency below 200ms",
        session,
    )
    brain.link(d2, inf, EdgeType.CAUSED_BY)

    # ── Traverse: reconstruct the full reasoning chain ───────────
    print("=== Full Reasoning Chain (from each fact) ===\n")

    for fact_id in [f1, f2, f3]:
        chain = brain.traverse(fact_id, max_depth=5)
        fact_node = chain[0] if chain else None
        if fact_node:
            print(f"Starting from: [{fact_node.id}] {fact_node.content}")
            for i, node in enumerate(chain[1:], 1):
                arrow = "  " + "──> " * i
                print(f"{arrow}[{node.id}] ({node.event_type}) {node.content}")
            print()

    # ── Traverse from the final inference back ───────────────────
    print("=== Reverse Traverse (from inference) ===\n")
    reverse_chain = brain.traverse(inf, max_depth=5)
    for node in reverse_chain:
        print(f"  [{node.id}] ({node.event_type}) {node.content}")

    print(f"\n=== Brain Info ===\n  {brain.info()}")


if __name__ == "__main__":
    main()
