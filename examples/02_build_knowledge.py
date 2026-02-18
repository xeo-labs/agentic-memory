#!/usr/bin/env python3
"""
02_build_knowledge.py — Multi-session knowledge building with corrections.

Demonstrates:
  - Building knowledge across multiple sessions
  - Using add_correction() to supersede outdated information
  - Using resolve() to follow the SUPERSEDES chain to the latest truth
  - Using impact() to see what a node has influenced
"""

from agentic_memory import Brain, EdgeType

def main():
    brain = Brain()

    # ── Session 1: Initial project facts ─────────────────────────
    print("=== Session 1: Project Discovery ===")
    s1 = "discovery"

    f_lang = brain.add_fact("Team has chosen React for the frontend", s1)
    f_db = brain.add_fact("Data will be stored in MongoDB", s1)
    f_deploy = brain.add_fact("Deployment target is AWS ECS", s1)

    print(f"  Added 3 project facts in session '{s1}'")

    # ── Session 2: Decisions based on session-1 facts ────────────
    print("\n=== Session 2: Architecture Decisions ===")
    s2 = "architecture"

    d_api = brain.add_decision("Build a REST API with Express.js", s2)
    d_ci = brain.add_decision("Use GitHub Actions for CI/CD pipeline", s2)

    # Decisions were caused by the facts
    brain.link(f_lang, d_api, EdgeType.CAUSED_BY)
    brain.link(f_db, d_api, EdgeType.CAUSED_BY)
    brain.link(f_deploy, d_ci, EdgeType.CAUSED_BY)

    print(f"  Added 2 decisions in session '{s2}'")

    # ── Session 3: Corrections — the team changed direction ──────
    print("\n=== Session 3: Tech Stack Pivot ===")
    s3 = "pivot"

    # The team switched from MongoDB to PostgreSQL
    f_db_new = brain.add_correction(
        "Data will be stored in PostgreSQL (switched from MongoDB)", s3,
        supersedes=f_db,
    )
    # And from Express.js to FastAPI (Python backend now)
    d_api_new = brain.add_correction(
        "Build a REST API with FastAPI instead of Express.js", s3,
        supersedes=d_api,
    )

    print(f"  Corrected DB choice:  old={f_db} -> new={f_db_new}")
    print(f"  Corrected API choice: old={d_api} -> new={d_api_new}")

    # ── Resolve: follow the SUPERSEDES chain to get latest truth ─
    print("\n=== Resolve: Latest Truth ===")
    resolved_db = brain.resolve(f_db)
    resolved_api = brain.resolve(d_api)
    print(f"  DB (resolved):  {resolved_db.content}")
    print(f"  API (resolved): {resolved_api.content}")

    # ── Impact: see what the original deployment fact influenced ──
    print("\n=== Impact Analysis: Deployment Fact ===")
    impact_result = brain.impact(f_deploy)
    print(f"  The deployment fact (id={f_deploy}) influenced:")
    for node in impact_result:
        print(f"    -> [{node.id}] {node.content}")

    print(f"\n=== Brain Info ===\n  {brain.info()}")


if __name__ == "__main__":
    main()
