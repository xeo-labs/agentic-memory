"""Scanner tests — tool detection in sandboxed environment."""

import json
from pathlib import Path
from unittest.mock import patch

import pytest

from amem_installer.scanner import Scanner, ToolStatus


def test_detect_claude_code_when_installed(sandbox):
    """Should detect Claude Code when config exists."""
    sandbox.install_claude_code()
    scanner = Scanner(sandbox.platform)
    tools = scanner.scan()
    claude = next(t for t in tools if t.tool_id == "claude_code")
    assert claude.status == ToolStatus.FOUND
    assert claude.config_path is not None


def test_detect_claude_code_not_installed(sandbox):
    """Should report NOT_FOUND when Claude Code isn't installed."""
    scanner = Scanner(sandbox.platform)
    tools = scanner.scan()
    claude = next(t for t in tools if t.tool_id == "claude_code")
    assert claude.status == ToolStatus.NOT_FOUND


def test_detect_claude_code_already_configured(sandbox):
    """Should detect already-configured Claude Code."""
    sandbox.install_claude_code({"mcpServers": {"agentic-memory": {"command": "amem"}}})
    scanner = Scanner(sandbox.platform)
    tools = scanner.scan()
    claude = next(t for t in tools if t.tool_id == "claude_code")
    assert claude.status == ToolStatus.ALREADY_CONFIGURED


def test_detect_cursor_when_installed(sandbox):
    """Should detect Cursor when config exists."""
    sandbox.install_cursor()
    scanner = Scanner(sandbox.platform)
    tools = scanner.scan()
    cursor = next(t for t in tools if t.tool_id == "cursor")
    assert cursor.status == ToolStatus.FOUND


def test_detect_continue_when_installed(sandbox):
    """Should detect Continue when config exists."""
    sandbox.install_continue()
    scanner = Scanner(sandbox.platform)
    tools = scanner.scan()
    cont = next(t for t in tools if t.tool_id == "continue")
    assert cont.status == ToolStatus.FOUND


def test_detect_openclaw_when_installed(sandbox):
    """Should detect OpenClaw when config exists."""
    sandbox.install_openclaw()
    scanner = Scanner(sandbox.platform)
    tools = scanner.scan()
    oc = next(t for t in tools if t.tool_id == "openclaw")
    assert oc.status == ToolStatus.FOUND


def test_detect_multiple_tools(sandbox):
    """Should detect multiple tools at once."""
    sandbox.install_claude_code()
    sandbox.install_cursor()
    sandbox.install_continue()
    scanner = Scanner(sandbox.platform)
    found = scanner.scan_found()
    assert len(found) >= 3


def test_scan_returns_all_tool_types(sandbox):
    """Should return entries for all supported tools even if not found."""
    scanner = Scanner(sandbox.platform)
    tools = scanner.scan()
    tool_ids = {t.tool_id for t in tools}
    assert "claude_code" in tool_ids
    assert "cursor" in tool_ids
    assert "ollama" in tool_ids
    assert "langchain" in tool_ids


def test_scanner_never_crashes(sandbox):
    """Should not crash on corrupted config files."""
    config_dir = sandbox.home / ".claude"
    config_dir.mkdir(parents=True, exist_ok=True)
    (config_dir / "claude_desktop_config.json").write_text("NOT JSON {{{")
    scanner = Scanner(sandbox.platform)
    tools = scanner.scan()  # Should not raise
    assert len(tools) > 0


def test_detect_framework_in_requirements(sandbox):
    """Should detect LangChain in requirements.txt."""
    proj = sandbox.create_project_with_requirements(["langchain==0.1.0", "openai"])
    with patch("pathlib.Path.cwd", return_value=proj):
        scanner = Scanner(sandbox.platform)
        tools = scanner.scan()
        lc = next(t for t in tools if t.tool_id == "langchain")
        assert lc.status == ToolStatus.FOUND


def test_detect_crewai_in_requirements(sandbox):
    """Should detect CrewAI in requirements.txt."""
    proj = sandbox.create_project_with_requirements(["crewai==0.2.0"])
    with patch("pathlib.Path.cwd", return_value=proj):
        scanner = Scanner(sandbox.platform)
        tools = scanner.scan()
        ca = next(t for t in tools if t.tool_id == "crewai")
        assert ca.status == ToolStatus.FOUND


def test_detect_autogen_in_requirements(sandbox):
    """Should detect AutoGen in requirements.txt."""
    proj = sandbox.create_project_with_requirements(["pyautogen==0.3.0"])
    with patch("pathlib.Path.cwd", return_value=proj):
        scanner = Scanner(sandbox.platform)
        tools = scanner.scan()
        ag = next(t for t in tools if t.tool_id == "autogen")
        assert ag.status == ToolStatus.FOUND
