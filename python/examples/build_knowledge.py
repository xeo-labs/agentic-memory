"""Build a knowledge graph across multiple sessions.

Demonstrates how memories accumulate and relate to each other.

Run: python examples/build_knowledge.py
"""

from agentic_memory import Brain, EdgeType

brain = Brain("knowledge.amem")

# === Session 1: Initial meeting ===
print("Session 1: Initial meeting")
f1 = brain.add_fact("User's name is Marcus", session=1, confidence=0.95)
f2 = brain.add_fact("Marcus is a backend developer", session=1, confidence=0.85)
d1 = brain.add_decision("Suggested Rust for their new CLI tool", session=1)
brain.link(d1, f2, EdgeType.CAUSED_BY)  # Decision was caused by knowing their role

# === Session 2: Follow-up ===
print("Session 2: Follow-up")
f3 = brain.add_fact("Marcus has 5 years of Python experience", session=2, confidence=0.9)
i1 = brain.add_inference("Marcus is likely comfortable with systems programming", session=2, confidence=0.7)
brain.link(i1, f2, EdgeType.SUPPORTS)
brain.link(i1, f3, EdgeType.SUPPORTS)

# === Session 3: Correction ===
print("Session 3: Correction")
c1 = brain.add_correction("Marcus actually works on frontend now", session=3, supersedes=f2)

# Show the knowledge graph
print("\n=== All Facts ===")
for fact in brain.facts(limit=10):
    print(f"  [ID:{fact.id}] {fact.content} (conf: {fact.confidence:.1f})")

print("\n=== Corrections ===")
for c in brain.corrections():
    print(f"  {c.content}")

# Resolve the original fact
resolved = brain.resolve(f2)
print(f"\nOriginal fact '{brain.get(f2).content}' resolves to: '{resolved.content}'")

# Impact analysis
print(f"\n=== Impact of fact '{brain.get(f1).content}' ===")
impact = brain.impact(f1)
print(f"  Total dependents: {impact.total_dependents}")

# Sessions
print("\n=== Sessions ===")
for s in brain.sessions():
    print(f"  Session {s.session_id}: {s.node_count} nodes")
