"""Brain API tests — requires the amem CLI binary."""

import pytest
from pathlib import Path

from agentic_memory import (
    Brain,
    Event,
    EventType,
    EdgeType,
    BrainInfo,
    SessionInfo,
    TraversalResult,
    ImpactResult,
    BrainNotFoundError,
    NodeNotFoundError,
    ValidationError,
    AmemNotFoundError,
)


# === Lifecycle ===

def test_brain_create_new(brain_path):
    """Should create a new brain file."""
    brain = Brain(brain_path)
    assert not brain.exists
    brain.create()
    assert brain.exists


def test_brain_auto_create_on_write(brain_path):
    """Should auto-create brain on first write when auto_create=True."""
    brain = Brain(brain_path, auto_create=True)
    brain.add_fact("test", session=1)
    assert brain.exists


def test_brain_info_empty(brain_path):
    """Should return empty info for a fresh brain."""
    brain = Brain(brain_path)
    brain.create()
    info = brain.info()
    assert info.node_count == 0
    assert info.edge_count == 0
    assert info.is_empty


def test_brain_info_typed(brain_path):
    """info() should return a typed BrainInfo."""
    brain = Brain(brain_path)
    brain.create()
    info = brain.info()
    assert isinstance(info, BrainInfo)
    assert isinstance(info.node_count, int)
    assert isinstance(info.dimension, int)


def test_brain_path_property(brain_path):
    """path property should return the resolved Path."""
    brain = Brain(brain_path)
    assert brain.path == Path(brain_path).expanduser().resolve()


# === Write Operations ===

def test_add_fact(brain):
    """add_fact should return an integer node ID."""
    brain.create()
    node_id = brain.add_fact("User likes Python", session=1)
    assert isinstance(node_id, int)
    assert node_id >= 0


def test_add_fact_with_confidence(brain):
    """add_fact should store the specified confidence."""
    brain.create()
    node_id = brain.add_fact("User might like Java", session=1, confidence=0.6)
    event = brain.get(node_id)
    assert event.confidence == pytest.approx(0.6, abs=0.01)


def test_add_decision(brain):
    """add_decision should create a DECISION node."""
    brain.create()
    node_id = brain.add_decision("Recommended PostgreSQL", session=1)
    event = brain.get(node_id)
    assert event.type == EventType.DECISION


def test_add_inference(brain):
    """add_inference should create an INFERENCE node."""
    brain.create()
    node_id = brain.add_inference("User is a senior developer", session=1)
    event = brain.get(node_id)
    assert event.type == EventType.INFERENCE


def test_add_correction(brain):
    """add_correction should create a CORRECTION node."""
    brain.create()
    old = brain.add_fact("Works at Company A", session=1)
    new = brain.add_correction("Works at Company B", session=2, supersedes=old)
    assert new != old
    info = brain.info()
    assert info.corrections >= 1


def test_add_skill(brain):
    """add_skill should create a SKILL node."""
    brain.create()
    node_id = brain.add_skill("Use tables for comparisons", session=1)
    event = brain.get(node_id)
    assert event.type == EventType.SKILL


def test_add_episode(brain):
    """add_episode should create an EPISODE node."""
    brain.create()
    node_id = brain.add_episode("Session 1 summary", session=1)
    event = brain.get(node_id)
    assert event.type == EventType.EPISODE


def test_link_nodes(brain):
    """link should create an edge between nodes."""
    brain.create()
    a = brain.add_fact("Fact A", session=1)
    b = brain.add_decision("Decision B", session=1)
    brain.link(a, b, EdgeType.CAUSED_BY)
    info = brain.info()
    assert info.edge_count >= 1


def test_link_with_string_type(brain):
    """link should accept string edge types."""
    brain.create()
    a = brain.add_fact("Fact A", session=1)
    b = brain.add_fact("Fact B", session=1)
    brain.link(a, b, "supports", weight=0.8)
    info = brain.info()
    assert info.edge_count >= 1


# === Read Operations ===

def test_get_node(brain):
    """get should return a typed Event."""
    brain.create()
    node_id = brain.add_fact("Test fact", session=1)
    event = brain.get(node_id)
    assert isinstance(event, Event)
    assert event.id == node_id
    assert event.content == "Test fact"
    assert event.session == 1


def test_get_nonexistent_raises(brain):
    """get should raise NodeNotFoundError for missing nodes."""
    brain.create()
    with pytest.raises(NodeNotFoundError):
        brain.get(99999)


def test_search_all(brain):
    """search should find all nodes."""
    brain.create()
    brain.add_fact("Fact 1", session=1)
    brain.add_decision("Decision 1", session=1)
    results = brain.search(limit=10)
    assert len(results) >= 2


def test_search_by_type(brain):
    """search should filter by type."""
    brain.create()
    brain.add_fact("Fact 1", session=1)
    brain.add_decision("Decision 1", session=1)
    facts = brain.search(types=["fact"])
    assert all(e.type == EventType.FACT for e in facts)


def test_search_by_session(brain):
    """search should filter by session."""
    brain.create()
    brain.add_fact("S1 fact", session=1)
    brain.add_fact("S2 fact", session=2)
    results = brain.search(sessions=[1])
    assert all(e.session == 1 for e in results)


def test_search_by_confidence(brain):
    """search should filter by minimum confidence."""
    brain.create()
    brain.add_fact("Sure thing", session=1, confidence=0.99)
    brain.add_fact("Maybe", session=1, confidence=0.3)
    results = brain.search(min_confidence=0.8)
    assert all(e.confidence >= 0.8 for e in results)


def test_search_limit(brain):
    """search should respect limit parameter."""
    brain.create()
    for i in range(30):
        brain.add_fact(f"Fact {i}", session=1)
    results = brain.search(limit=10)
    assert len(results) <= 10


def test_facts_convenience(brain):
    """facts() should return only facts."""
    brain.create()
    brain.add_fact("A fact", session=1)
    brain.add_decision("A decision", session=1)
    facts = brain.facts()
    assert all(e.type == EventType.FACT for e in facts)


def test_decisions_convenience(brain):
    """decisions() should return only decisions."""
    brain.create()
    brain.add_decision("A decision", session=1)
    decs = brain.decisions()
    assert len(decs) >= 1
    assert all(e.type == EventType.DECISION for e in decs)


# === Graph Operations ===

def test_traverse_basic(brain):
    """traverse should return a TraversalResult."""
    brain.create()
    a = brain.add_fact("Root", session=1)
    b = brain.add_decision("Branch", session=1)
    brain.link(b, a, "caused_by")
    result = brain.traverse(b, edges=["caused_by"], direction="forward")
    assert isinstance(result, TraversalResult)
    assert a in result.visited or b in result.visited


def test_traverse_returns_typed(brain):
    """traverse should return proper types."""
    brain.create()
    a = brain.add_fact("Root", session=1)
    result = brain.traverse(a)
    assert isinstance(result, TraversalResult)
    assert isinstance(result.visited, list)
    assert isinstance(result.count, int)


def test_resolve_correction_chain(brain):
    """resolve should follow SUPERSEDES chain."""
    brain.create()
    old = brain.add_fact("Version 1", session=1)
    new = brain.add_correction("Version 2", session=2, supersedes=old)
    resolved = brain.resolve(old)
    assert resolved.content == "Version 2"


def test_resolve_no_correction(brain):
    """resolve should return the node itself if no correction exists."""
    brain.create()
    node_id = brain.add_fact("Current", session=1)
    resolved = brain.resolve(node_id)
    assert resolved.id == node_id


def test_impact_analysis(brain):
    """impact should return an ImpactResult."""
    brain.create()
    fact = brain.add_fact("Foundation", session=1)
    dec = brain.add_decision("Built on it", session=1)
    brain.link(dec, fact, "caused_by")
    impact = brain.impact(fact)
    assert isinstance(impact, ImpactResult)
    assert impact.total_dependents >= 1


# === Session Operations ===

def test_sessions_list(brain):
    """sessions should list all sessions."""
    brain.create()
    brain.add_fact("S1", session=1)
    brain.add_fact("S2", session=2)
    sessions = brain.sessions()
    assert len(sessions) >= 2
    assert all(isinstance(s, SessionInfo) for s in sessions)


def test_session_events(brain):
    """session_events should return events from a specific session."""
    brain.create()
    brain.add_fact("S1 F1", session=1)
    brain.add_fact("S1 F2", session=1)
    brain.add_fact("S2 F1", session=2)
    events = brain.session_events(1)
    assert len(events) >= 2
    assert all(e.session == 1 for e in events)


# === Statistics ===

def test_stats(brain):
    """stats should return a dict with statistics."""
    brain.create()
    brain.add_fact("Test", session=1)
    stats = brain.stats()
    assert isinstance(stats, dict)
    assert len(stats) > 0


# === Error Handling ===

def test_brain_not_found_error(brain_path):
    """Should raise BrainNotFoundError for missing file."""
    brain = Brain(brain_path, auto_create=False)
    with pytest.raises(BrainNotFoundError):
        brain.info()


def test_cli_not_found_error():
    """Should raise AmemNotFoundError for bad binary path."""
    with pytest.raises(AmemNotFoundError):
        Brain("test.amem", amem_binary="/nonexistent/amem")


def test_validation_error_self_edge(brain):
    """Should raise ValidationError for self-edges."""
    brain.create()
    node = brain.add_fact("Self", session=1)
    with pytest.raises(ValidationError):
        brain.link(node, node, "related_to")
