"""CLI bridge tests — verify binary discovery and command execution."""

import os
import pytest
from pathlib import Path


def test_find_amem_binary():
    """Should find amem in PATH or common locations."""
    from agentic_memory.cli_bridge import find_amem_binary
    path = find_amem_binary()
    assert path.exists()
    assert os.access(str(path), os.X_OK)


def test_find_amem_binary_env_override(tmp_path, monkeypatch):
    """Should use AMEM_BINARY env var if set."""
    from agentic_memory.cli_bridge import find_amem_binary

    # Find the real binary first
    real_binary = find_amem_binary()

    # Set env var to a known good path
    monkeypatch.setenv("AMEM_BINARY", str(real_binary))
    result = find_amem_binary()
    assert result == real_binary


def test_find_amem_binary_not_found():
    """Should raise AmemNotFoundError when binary doesn't exist."""
    from agentic_memory.cli_bridge import find_amem_binary
    from agentic_memory.errors import AmemNotFoundError

    with pytest.raises(AmemNotFoundError):
        find_amem_binary(override="/nonexistent/amem")


def test_run_command_success():
    """Should execute amem --help successfully."""
    from agentic_memory.cli_bridge import run_command, find_amem_binary
    binary = find_amem_binary()
    output = run_command(binary, ["--help"])
    assert len(output) > 0
    assert "amem" in output.lower() or "AgenticMemory" in output


def test_run_command_failure():
    """Should raise CLIError on bad command."""
    from agentic_memory.cli_bridge import run_command, find_amem_binary
    from agentic_memory.errors import CLIError
    binary = find_amem_binary()
    with pytest.raises(CLIError):
        run_command(binary, ["info", "/nonexistent/file.amem"])


def test_parse_node_id_text_format():
    """Should parse node ID from text output."""
    from agentic_memory.cli_bridge import parse_node_id
    assert parse_node_id("Added node 42 (fact) to brain.amem") == 42


def test_parse_node_id_json_format():
    """Should parse node ID from JSON output."""
    from agentic_memory.cli_bridge import parse_node_id
    assert parse_node_id('{"id": 42}') == 42


def test_parse_node_id_failure():
    """Should raise CLIError on unparseable output."""
    from agentic_memory.cli_bridge import parse_node_id
    from agentic_memory.errors import CLIError
    with pytest.raises(CLIError):
        parse_node_id("some garbage output")
