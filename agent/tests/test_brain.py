"""Tests for the Brain CLI wrapper (amem_agent.brain).

Every test mocks subprocess.run to avoid requiring the real ``amem`` binary.
"""

from __future__ import annotations

import json
import subprocess
from unittest.mock import MagicMock, patch

import pytest

from amem_agent.brain import Brain, BrainError, BrainInfo


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _make_run_result(
    stdout: str = "",
    stderr: str = "",
    returncode: int = 0,
) -> subprocess.CompletedProcess:
    return subprocess.CompletedProcess(
        args=["amem"],
        returncode=returncode,
        stdout=stdout,
        stderr=stderr,
    )


def _make_brain(**kwargs) -> Brain:
    """Create a Brain instance with the binary-verification step bypassed."""
    with patch.object(Brain, "_verify_binary"):
        return Brain(brain_path="/tmp/test.amem", **kwargs)


# ---------------------------------------------------------------------------
# Lifecycle
# ---------------------------------------------------------------------------


class TestEnsureExists:
    def test_ensure_exists_creates_file(self, tmp_path):
        """ensure_exists should invoke 'amem create <path>' when the file is absent."""
        brain_path = str(tmp_path / "new.amem")

        with patch.object(Brain, "_verify_binary"):
            brain = Brain(brain_path=brain_path)

        with patch("amem_agent.brain.subprocess.run") as mock_run:
            mock_run.return_value = _make_run_result(stdout="Created new brain")
            brain.ensure_exists()

        mock_run.assert_called_once()
        call_args = mock_run.call_args[0][0]
        assert "create" in call_args
        assert brain_path in call_args


# ---------------------------------------------------------------------------
# add_* methods
# ---------------------------------------------------------------------------


class TestAddFact:
    def test_add_fact_parses_node_id(self):
        """add_fact should parse the node ID from 'Added node 42 (fact)...'."""
        brain = _make_brain()

        with patch("amem_agent.brain.subprocess.run") as mock_run:
            mock_run.return_value = _make_run_result(
                stdout="Added node 42 (fact) to /tmp/test.amem"
            )
            node_id = brain.add_fact("Python is great", session_id=1, confidence=0.9)

        assert node_id == 42
        call_args = mock_run.call_args[0][0]
        assert "add" in call_args
        assert "fact" in call_args
        assert "Python is great" in call_args
        assert "--session" in call_args
        assert "1" in call_args
        assert "--confidence" in call_args
        assert "0.9" in call_args


class TestAddDecision:
    def test_add_decision_returns_id(self):
        brain = _make_brain()

        with patch("amem_agent.brain.subprocess.run") as mock_run:
            mock_run.return_value = _make_run_result(
                stdout="Added node 7 (decision) to /tmp/test.amem"
            )
            node_id = brain.add_decision("Use Postgres", session_id=2, confidence=0.85)

        assert node_id == 7
        call_args = mock_run.call_args[0][0]
        assert "decision" in call_args


class TestAddCorrection:
    def test_add_correction_with_supersedes(self):
        brain = _make_brain()

        with patch("amem_agent.brain.subprocess.run") as mock_run:
            mock_run.return_value = _make_run_result(
                stdout="Added node 99 (correction) to /tmp/test.amem"
            )
            node_id = brain.add_correction(
                "Actually use MySQL", session_id=3, supersedes_id=7
            )

        assert node_id == 99
        call_args = mock_run.call_args[0][0]
        assert "correction" in call_args
        assert "--supersedes" in call_args
        assert "7" in call_args
        assert "--session" in call_args
        assert "3" in call_args


# ---------------------------------------------------------------------------
# link
# ---------------------------------------------------------------------------


class TestLink:
    def test_link_calls_correct_args(self):
        brain = _make_brain()

        with patch("amem_agent.brain.subprocess.run") as mock_run:
            mock_run.return_value = _make_run_result(stdout="Linked 1 -> 2")
            brain.link(source_id=1, target_id=2, edge_type="supports", weight=0.75)

        call_args = mock_run.call_args[0][0]
        assert "link" in call_args
        assert "1" in call_args
        assert "2" in call_args
        assert "supports" in call_args
        assert "--weight" in call_args
        assert "0.75" in call_args


# ---------------------------------------------------------------------------
# search
# ---------------------------------------------------------------------------


class TestSearch:
    def test_search_returns_list(self):
        brain = _make_brain()
        nodes = [
            {"id": 1, "event_type": "fact", "content": "User likes Python"},
            {"id": 2, "event_type": "decision", "content": "Use Flask"},
        ]

        with patch("amem_agent.brain.subprocess.run") as mock_run:
            mock_run.return_value = _make_run_result(stdout=json.dumps(nodes))
            result = brain.search()

        assert isinstance(result, list)
        assert len(result) == 2
        assert result[0]["id"] == 1
        assert result[1]["content"] == "Use Flask"

    def test_search_with_filters(self):
        brain = _make_brain()

        with patch("amem_agent.brain.subprocess.run") as mock_run:
            mock_run.return_value = _make_run_result(stdout=json.dumps([]))
            brain.search(
                event_types=["fact", "decision"],
                session_ids=[1, 2],
                min_confidence=0.5,
                sort="confidence",
                limit=10,
            )

        call_args = mock_run.call_args[0][0]
        assert "--event-types" in call_args
        idx = call_args.index("--event-types")
        assert call_args[idx + 1] == "fact,decision"

        assert "--session" in call_args
        idx = call_args.index("--session")
        assert call_args[idx + 1] == "1,2"

        assert "--min-confidence" in call_args
        idx = call_args.index("--min-confidence")
        assert call_args[idx + 1] == "0.5"

        assert "--sort" in call_args
        idx = call_args.index("--sort")
        assert call_args[idx + 1] == "confidence"

        assert "--limit" in call_args
        idx = call_args.index("--limit")
        assert call_args[idx + 1] == "10"


# ---------------------------------------------------------------------------
# info
# ---------------------------------------------------------------------------


class TestInfo:
    def test_info_returns_brain_info(self):
        brain = _make_brain()
        info_json = {
            "version": 2,
            "dimension": 128,
            "nodes": 50,
            "edges": 30,
            "sessions": 5,
            "node_types": {
                "facts": 20,
                "decisions": 10,
                "inferences": 8,
                "corrections": 5,
                "skills": 4,
                "episodes": 3,
            },
            "file_size": 12345,
        }

        with patch("amem_agent.brain.subprocess.run") as mock_run:
            mock_run.return_value = _make_run_result(stdout=json.dumps(info_json))
            result = brain.info()

        assert isinstance(result, BrainInfo)
        assert result.version == 2
        assert result.dimension == 128
        assert result.node_count == 50
        assert result.edge_count == 30
        assert result.session_count == 5
        assert result.facts == 20
        assert result.decisions == 10
        assert result.inferences == 8
        assert result.corrections == 5
        assert result.skills == 4
        assert result.episodes == 3
        assert result.file_size_bytes == 12345


# ---------------------------------------------------------------------------
# get_node
# ---------------------------------------------------------------------------


class TestGetNode:
    def test_get_node_returns_dict(self):
        brain = _make_brain()
        node_json = {
            "id": 42,
            "event_type": "fact",
            "content": "User prefers dark mode",
            "confidence": 0.95,
            "session_id": 1,
        }

        with patch("amem_agent.brain.subprocess.run") as mock_run:
            mock_run.return_value = _make_run_result(stdout=json.dumps(node_json))
            result = brain.get_node(42)

        assert isinstance(result, dict)
        assert result["id"] == 42
        assert result["content"] == "User prefers dark mode"


# ---------------------------------------------------------------------------
# Error handling
# ---------------------------------------------------------------------------


class TestBrainError:
    def test_brain_error_on_bad_exit_code(self):
        brain = _make_brain()

        with patch("amem_agent.brain.subprocess.run") as mock_run:
            mock_run.return_value = _make_run_result(
                stdout="",
                stderr="Error: brain file not found",
                returncode=1,
            )
            with pytest.raises(BrainError) as exc_info:
                brain.search()

        assert exc_info.value.returncode == 1
        assert "brain file not found" in exc_info.value.stderr


# ---------------------------------------------------------------------------
# _run_json
# ---------------------------------------------------------------------------


class TestRunJson:
    def test_run_json_parses_output(self):
        brain = _make_brain()
        expected = {"key": "value", "count": 42}

        with patch("amem_agent.brain.subprocess.run") as mock_run:
            mock_run.return_value = _make_run_result(stdout=json.dumps(expected))
            result = brain._run_json(["info", "/tmp/test.amem"])

        assert result == expected

        # Verify that --format json is prepended to args
        call_args = mock_run.call_args[0][0]
        assert call_args[1] == "--format"
        assert call_args[2] == "json"


# ---------------------------------------------------------------------------
# Binary verification
# ---------------------------------------------------------------------------


class TestVerifyBinary:
    def test_verify_binary_success(self):
        """When 'which amem' succeeds, no exception should be raised."""
        with patch("amem_agent.brain.subprocess.run") as mock_run:
            mock_run.return_value = _make_run_result(stdout="/usr/local/bin/amem")
            # This should not raise
            brain = Brain(brain_path="/tmp/test.amem", amem_binary="amem")
        assert brain.brain_path == "/tmp/test.amem"

    def test_verify_binary_failure(self):
        """When 'which amem' fails, BrainError should be raised."""
        with patch("amem_agent.brain.subprocess.run") as mock_run:
            mock_run.side_effect = subprocess.CalledProcessError(
                returncode=1, cmd=["which", "amem"]
            )
            with pytest.raises(BrainError) as exc_info:
                Brain(brain_path="/tmp/test.amem", amem_binary="amem")

        assert "not found" in exc_info.value.stderr
