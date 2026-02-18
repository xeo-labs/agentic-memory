"""Basic usage — the simplest possible AgenticMemory example.

Run: python examples/basic_usage.py
"""

from agentic_memory import Brain

# Create a brain (file is created automatically)
brain = Brain("example.amem")

# Add some facts
brain.add_fact("User's name is Alice", session=1, confidence=0.95)
brain.add_fact("User prefers Python over JavaScript", session=1, confidence=0.8)
brain.add_fact("User works at TechCorp", session=1, confidence=0.9)

# Add a decision
brain.add_decision("Recommended FastAPI for their API project", session=1)

# Add a skill
brain.add_skill("User likes code examples with type hints", session=1)

# Query facts
print("=== Facts ===")
for fact in brain.facts():
    print(f"  [{fact.confidence:.1f}] {fact.content}")

# Get brain stats
info = brain.info()
print(f"\nBrain has {info.node_count} nodes, {info.edge_count} edges")
print(f"  Facts: {info.facts}, Decisions: {info.decisions}, Skills: {info.skills}")
