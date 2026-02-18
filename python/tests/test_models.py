"""Data model tests — pure Python, no CLI needed."""

import pytest
from agentic_memory.models import (
    Event,
    Edge,
    EventType,
    EdgeType,
    BrainInfo,
    SessionInfo,
    TraversalResult,
    ImpactResult,
    parse_event,
)


def test_event_type_enum_values():
    """EventType enum values should match CLI strings."""
    assert EventType.FACT.value == "fact"
    assert EventType.DECISION.value == "decision"
    assert EventType.INFERENCE.value == "inference"
    assert EventType.CORRECTION.value == "correction"
    assert EventType.SKILL.value == "skill"
    assert EventType.EPISODE.value == "episode"


def test_event_type_string_behavior():
    """str, Enum should work as strings."""
    assert EventType.FACT == "fact"
    assert EventType.DECISION == "decision"


def test_edge_type_enum_values():
    """EdgeType enum values should match CLI strings."""
    assert EdgeType.CAUSED_BY.value == "caused_by"
    assert EdgeType.SUPPORTS.value == "supports"
    assert EdgeType.CONTRADICTS.value == "contradicts"
    assert EdgeType.SUPERSEDES.value == "supersedes"


def test_event_creation():
    """Event should be creatable with required fields."""
    e = Event(id=1, type=EventType.FACT, content="Test", session=1, confidence=0.9)
    assert e.id == 1
    assert e.type == EventType.FACT
    assert e.content == "Test"
    assert e.session == 1
    assert e.confidence == 0.9


def test_event_frozen():
    """Event should be immutable (frozen dataclass)."""
    e = Event(id=1, type=EventType.FACT, content="Test", session=1, confidence=0.9)
    with pytest.raises(AttributeError):
        e.content = "Modified"  # type: ignore[misc]


def test_event_defaults():
    """Event should have sensible defaults for optional fields."""
    e = Event(id=1, type=EventType.FACT, content="Test", session=1, confidence=0.9)
    assert e.access_count == 0
    assert e.decay_score == 1.0
    assert e.created_at == ""
    assert e.edges_out == 0
    assert e.edges_in == 0


def test_edge_creation():
    """Edge should be creatable with required fields."""
    edge = Edge(source=1, target=2, type=EdgeType.CAUSED_BY)
    assert edge.source == 1
    assert edge.target == 2
    assert edge.type == EdgeType.CAUSED_BY
    assert edge.weight == 1.0


def test_brain_info_is_empty():
    """BrainInfo.is_empty should reflect node_count."""
    info = BrainInfo(
        path="t", version=1, dimension=128,
        node_count=0, edge_count=0, session_count=0, file_size=0,
    )
    assert info.is_empty

    info2 = BrainInfo(
        path="t", version=1, dimension=128,
        node_count=5, edge_count=0, session_count=1, file_size=100,
    )
    assert not info2.is_empty


def test_session_info_creation():
    """SessionInfo should store session data."""
    si = SessionInfo(session_id=3, node_count=10)
    assert si.session_id == 3
    assert si.node_count == 10


def test_traversal_result_count():
    """TraversalResult.count should return number of visited nodes."""
    tr = TraversalResult(
        visited=[1, 2, 3],
        depths={1: 0, 2: 1, 3: 1},
        start_id=1,
        edge_types_followed=["caused_by"],
        direction="forward",
    )
    assert tr.count == 3


def test_traversal_result_at_depth():
    """TraversalResult.at_depth should filter by depth."""
    tr = TraversalResult(
        visited=[1, 2, 3, 4],
        depths={1: 0, 2: 1, 3: 1, 4: 2},
        start_id=1,
        edge_types_followed=[],
        direction="forward",
    )
    assert tr.at_depth(1) == [2, 3]
    assert tr.at_depth(0) == [1]
    assert tr.at_depth(2) == [4]


def test_impact_result_has_dependents():
    """ImpactResult.has_dependents should reflect total_dependents."""
    ir = ImpactResult(
        root_id=1, dependents=[2, 3],
        affected_decisions=1, affected_inferences=1,
        total_dependents=2, max_depth=1,
    )
    assert ir.has_dependents

    ir2 = ImpactResult(
        root_id=1, dependents=[],
        affected_decisions=0, affected_inferences=0,
        total_dependents=0, max_depth=0,
    )
    assert not ir2.has_dependents


def test_parse_event_from_dict():
    """parse_event should create an Event from a CLI JSON dict."""
    data = {"id": 5, "type": "fact", "content": "Hello", "session": 1, "confidence": 0.9}
    e = parse_event(data)
    assert e.id == 5
    assert e.type == EventType.FACT
    assert e.content == "Hello"
    assert e.session == 1
    assert e.confidence == 0.9


def test_parse_event_handles_missing_keys():
    """parse_event should handle minimal dicts with defaults."""
    data = {"id": 5}
    e = parse_event(data)
    assert e.id == 5
    assert e.confidence == 1.0
    assert e.content == ""
    assert e.session == 0


def test_parse_event_handles_alternate_keys():
    """parse_event should handle alternate key names from CLI."""
    data = {
        "node_id": 5,
        "event_type": "decision",
        "content": "X",
        "session_id": 2,
        "confidence": 0.7,
    }
    e = parse_event(data)
    assert e.id == 5
    assert e.type == EventType.DECISION
    assert e.session == 2
