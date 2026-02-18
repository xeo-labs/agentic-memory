"""Configurator tests — config generation in sandboxed environment."""

import json
from pathlib import Path

import pytest

from amem_installer.configurators import (
    MCPConfigurator,
    ContinueConfigurator,
    OllamaConfigurator,
    LangChainConfigurator,
    get_configurator,
    ConfigResult,
)
from amem_installer.scanner import DetectedTool, ToolStatus


# === MCP Configurator Tests ===

def test_mcp_configure_claude_code(sandbox):
    """Should add MCP server entry to Claude Code config."""
    config_path = sandbox.install_claude_code()
    tool = DetectedTool(
        name="Claude Code", tool_id="claude_code", status=ToolStatus.FOUND,
        config_path=config_path, version=None, integration_type="mcp", notes=None,
    )
    amem_binary = sandbox.create_amem_binary()
    brain_path = sandbox.platform.brain_path

    configurator = MCPConfigurator()
    report = configurator.configure(tool, brain_path, amem_binary)

    assert report.result == ConfigResult.SUCCESS
    assert report.backup_path is not None

    config = json.loads(config_path.read_text())
    assert "agentic-memory" in config["mcpServers"]
    assert config["mcpServers"]["agentic-memory"]["command"] == str(amem_binary)


def test_mcp_configure_preserves_existing_servers(sandbox):
    """Should preserve existing MCP servers when adding."""
    existing = {"mcpServers": {"some-other-server": {"command": "other"}}}
    config_path = sandbox.install_claude_code(existing)
    tool = DetectedTool(
        name="Claude Code", tool_id="claude_code", status=ToolStatus.FOUND,
        config_path=config_path, version=None, integration_type="mcp", notes=None,
    )

    configurator = MCPConfigurator()
    report = configurator.configure(tool, sandbox.platform.brain_path, sandbox.create_amem_binary())

    config = json.loads(config_path.read_text())
    assert "some-other-server" in config["mcpServers"]
    assert "agentic-memory" in config["mcpServers"]


def test_mcp_configure_already_configured(sandbox):
    """Should return ALREADY_CONFIGURED if already set up."""
    existing = {"mcpServers": {"agentic-memory": {"command": "amem"}}}
    config_path = sandbox.install_claude_code(existing)
    tool = DetectedTool(
        name="Claude Code", tool_id="claude_code", status=ToolStatus.FOUND,
        config_path=config_path, version=None, integration_type="mcp", notes=None,
    )

    configurator = MCPConfigurator()
    report = configurator.configure(tool, sandbox.platform.brain_path, sandbox.create_amem_binary())
    assert report.result == ConfigResult.ALREADY_CONFIGURED


def test_mcp_configure_creates_backup(sandbox):
    """Should create a backup of the original config."""
    config_path = sandbox.install_claude_code()
    original_content = config_path.read_text()

    tool = DetectedTool(
        name="Claude Code", tool_id="claude_code", status=ToolStatus.FOUND,
        config_path=config_path, version=None, integration_type="mcp", notes=None,
    )

    configurator = MCPConfigurator()
    report = configurator.configure(tool, sandbox.platform.brain_path, sandbox.create_amem_binary())

    assert report.backup_path is not None
    backup = Path(report.backup_path)
    assert backup.exists()
    assert backup.read_text() == original_content


def test_mcp_configure_dry_run(sandbox):
    """Dry run should not modify the config file."""
    config_path = sandbox.install_claude_code()
    original_content = config_path.read_text()

    tool = DetectedTool(
        name="Claude Code", tool_id="claude_code", status=ToolStatus.FOUND,
        config_path=config_path, version=None, integration_type="mcp", notes=None,
    )

    configurator = MCPConfigurator()
    report = configurator.configure(
        tool, sandbox.platform.brain_path, sandbox.create_amem_binary(), dry_run=True,
    )

    assert report.result == ConfigResult.SUCCESS
    assert config_path.read_text() == original_content


def test_mcp_unconfigure_restores_backup(sandbox):
    """Unconfigure should restore config from backup."""
    config_path = sandbox.install_claude_code()
    original_content = config_path.read_text()

    tool = DetectedTool(
        name="Claude Code", tool_id="claude_code", status=ToolStatus.FOUND,
        config_path=config_path, version=None, integration_type="mcp", notes=None,
    )

    configurator = MCPConfigurator()
    report = configurator.configure(tool, sandbox.platform.brain_path, sandbox.create_amem_binary())

    report2 = configurator.unconfigure(tool, report.backup_path)
    assert report2.result == ConfigResult.SUCCESS
    assert config_path.read_text() == original_content


def test_mcp_unconfigure_surgical_removal(sandbox):
    """Unconfigure without backup should surgically remove the entry."""
    existing = {"mcpServers": {"agentic-memory": {"command": "amem"}, "other": {"command": "other"}}}
    config_path = sandbox.install_claude_code(existing)

    tool = DetectedTool(
        name="Claude Code", tool_id="claude_code", status=ToolStatus.FOUND,
        config_path=config_path, version=None, integration_type="mcp", notes=None,
    )

    configurator = MCPConfigurator()
    report = configurator.unconfigure(tool, None)

    config = json.loads(config_path.read_text())
    assert "agentic-memory" not in config["mcpServers"]
    assert "other" in config["mcpServers"]


def test_mcp_verify(sandbox):
    """verify() should return True when configured."""
    existing = {"mcpServers": {"agentic-memory": {"command": "amem"}}}
    config_path = sandbox.install_claude_code(existing)
    tool = DetectedTool(
        name="Claude Code", tool_id="claude_code", status=ToolStatus.FOUND,
        config_path=config_path, version=None, integration_type="mcp", notes=None,
    )
    configurator = MCPConfigurator()
    assert configurator.verify(tool) is True


def test_mcp_verify_not_configured(sandbox):
    """verify() should return False when not configured."""
    config_path = sandbox.install_claude_code()
    tool = DetectedTool(
        name="Claude Code", tool_id="claude_code", status=ToolStatus.FOUND,
        config_path=config_path, version=None, integration_type="mcp", notes=None,
    )
    configurator = MCPConfigurator()
    assert configurator.verify(tool) is False


# === Continue Configurator Tests ===

def test_continue_configure(sandbox):
    """Should add context provider to Continue config."""
    config_path = sandbox.install_continue()
    tool = DetectedTool(
        name="Continue", tool_id="continue", status=ToolStatus.FOUND,
        config_path=config_path, version=None, integration_type="config", notes=None,
    )

    configurator = ContinueConfigurator()
    report = configurator.configure(tool, sandbox.platform.brain_path, sandbox.create_amem_binary())
    assert report.result == ConfigResult.SUCCESS

    config = json.loads(config_path.read_text())
    providers = config.get("contextProviders", [])
    amem_providers = [p for p in providers if p.get("name") == "agentic-memory"]
    assert len(amem_providers) == 1


# === Ollama Configurator Tests ===

def test_ollama_configure(sandbox):
    """Should create wrapper script and config file."""
    tool = DetectedTool(
        name="Ollama", tool_id="ollama", status=ToolStatus.RUNNING,
        config_path=None, version=None, integration_type="service", notes=None,
    )

    brain_path = sandbox.platform.brain_path
    brain_path.parent.mkdir(parents=True, exist_ok=True)

    configurator = OllamaConfigurator()
    report = configurator.configure(tool, brain_path, sandbox.create_amem_binary())
    assert report.result == ConfigResult.SUCCESS

    wrapper = brain_path.parent / "ollama-amem"
    assert wrapper.exists()
    assert wrapper.stat().st_mode & 0o111  # Is executable


# === Framework Configurator Tests ===

def test_langchain_configure_prints_instructions(sandbox):
    """Should return instructions for LangChain integration."""
    tool = DetectedTool(
        name="LangChain", tool_id="langchain", status=ToolStatus.FOUND,
        config_path=None, version=None, integration_type="framework", notes=None,
    )

    configurator = LangChainConfigurator()
    report = configurator.configure(tool, sandbox.platform.brain_path, sandbox.create_amem_binary())
    assert report.result == ConfigResult.SUCCESS
    assert "from agentic_memory" in report.message


# === Configurator Factory Tests ===

def test_get_configurator_mcp_tools():
    """Factory should return MCPConfigurator for MCP tools."""
    for tool_id in ["claude_code", "claude_desktop", "cursor", "windsurf"]:
        tool = DetectedTool(
            name="X", tool_id=tool_id, status=ToolStatus.FOUND,
            config_path=None, version=None, integration_type="mcp", notes=None,
        )
        configurator = get_configurator(tool)
        assert isinstance(configurator, MCPConfigurator)


def test_get_configurator_returns_none_for_unknown():
    """Factory should return None for unknown tools."""
    tool = DetectedTool(
        name="X", tool_id="unknown_tool", status=ToolStatus.FOUND,
        config_path=None, version=None, integration_type="unknown", notes=None,
    )
    assert get_configurator(tool) is None
