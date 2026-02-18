"""Query brain — demonstrate all query types.

Run: python examples/query_brain.py
"""

from agentic_memory import Brain, EventType, EdgeType

brain = Brain("query_demo.amem")

# Build some data first
f1 = brain.add_fact("User likes dark mode", session=1, confidence=0.9)
f2 = brain.add_fact("User prefers vim keybindings", session=1, confidence=0.85)
f3 = brain.add_fact("User's timezone is EST", session=2, confidence=0.95)
d1 = brain.add_decision("Set editor theme to dark", session=1, confidence=0.9)
d2 = brain.add_decision("Enabled vim mode in IDE", session=2)
i1 = brain.add_inference("User is a power user", session=2, confidence=0.7)
brain.link(d1, f1, EdgeType.CAUSED_BY)
brain.link(d2, f2, EdgeType.CAUSED_BY)
brain.link(i1, f1, EdgeType.SUPPORTS)
brain.link(i1, f2, EdgeType.SUPPORTS)

# === Search queries ===
print("=== Search by type ===")
facts = brain.search(types=[EventType.FACT], limit=5)
for f in facts:
    print(f"  {f.content}")

print("\n=== Search by session ===")
s2_events = brain.search(sessions=[2])
for e in s2_events:
    print(f"  [{e.type.value}] {e.content}")

print("\n=== Search by confidence ===")
high_conf = brain.search(min_confidence=0.9)
for e in high_conf:
    print(f"  [{e.confidence:.1f}] {e.content}")

# === Graph traversal ===
print("\n=== Traverse from inference ===")
result = brain.traverse(i1, direction="forward")
print(f"  Visited {result.count} nodes:")
for nid in result.visited:
    node = brain.get(nid)
    print(f"    [{node.type.value}] {node.content}")

# === Impact analysis ===
print(f"\n=== Impact of '{brain.get(f1).content}' ===")
impact = brain.impact(f1)
print(f"  Dependents: {impact.total_dependents}")
print(f"  Affected decisions: {impact.affected_decisions}")
print(f"  Affected inferences: {impact.affected_inferences}")

# === Context (neighborhood) ===
print(f"\n=== Context around '{brain.get(d1).content}' ===")
neighbors = brain.context(d1, depth=1)
for n in neighbors:
    print(f"  [{n.type.value}] {n.content}")

# === Stats ===
print("\n=== Brain Stats ===")
stats = brain.stats()
for key, value in stats.items():
    print(f"  {key}: {value}")
