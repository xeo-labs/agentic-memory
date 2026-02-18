"""CLI integration tests — end-to-end flows in sandboxed environment."""

import json
from pathlib import Path

import pytest

from amem_installer.configurators import MCPConfigurator, ConfigResult, get_configurator
from amem_installer.scanner import Scanner, ToolStatus, DetectedTool


def test_install_dry_run(sandbox):
    """Dry run should not modify any config files."""
    sandbox.install_claude_code()
    sandbox.install_cursor()

    original_claude = (sandbox.home / ".claude" / "claude_desktop_config.json").read_text()
    original_cursor = (sandbox.home / ".cursor" / "mcp.json").read_text()

    amem_binary = sandbox.create_amem_binary()
    brain_path = sandbox.platform.brain_path

    scanner = Scanner(sandbox.platform)
    found = scanner.scan_found()

    for tool in found:
        configurator = get_configurator(tool)
        if configurator:
            configurator.configure(tool, brain_path, amem_binary, dry_run=True)

    assert (sandbox.home / ".claude" / "claude_desktop_config.json").read_text() == original_claude
    assert (sandbox.home / ".cursor" / "mcp.json").read_text() == original_cursor


def test_install_configures_found_tools(sandbox):
    """Install should configure all found tools."""
    sandbox.install_claude_code()
    amem_binary = sandbox.create_amem_binary()
    brain_path = sandbox.platform.brain_path
    brain_path.parent.mkdir(parents=True, exist_ok=True)

    scanner = Scanner(sandbox.platform)
    tools = scanner.scan_found()
    assert len(tools) >= 1

    for tool in tools:
        configurator = get_configurator(tool)
        if configurator:
            report = configurator.configure(tool, brain_path, amem_binary)
            assert report.result in (ConfigResult.SUCCESS, ConfigResult.ALREADY_CONFIGURED)


def test_uninstall_restores_configs(sandbox):
    """Install then uninstall should restore original configs."""
    config_path = sandbox.install_claude_code()
    original = config_path.read_text()

    amem_binary = sandbox.create_amem_binary()
    brain_path = sandbox.platform.brain_path
    brain_path.parent.mkdir(parents=True, exist_ok=True)

    tool = DetectedTool(
        name="Claude Code", tool_id="claude_code", status=ToolStatus.FOUND,
        config_path=config_path, version=None, integration_type="mcp", notes=None,
    )

    configurator = MCPConfigurator()
    report = configurator.configure(tool, brain_path, amem_binary)
    assert report.result == ConfigResult.SUCCESS

    # Uninstall
    report2 = configurator.unconfigure(tool, report.backup_path)
    assert report2.result == ConfigResult.SUCCESS
    assert config_path.read_text() == original


def test_full_scan_configure_verify_cycle(sandbox):
    """Full cycle: scan → configure → verify."""
    sandbox.install_claude_code()
    sandbox.install_cursor()
    amem_binary = sandbox.create_amem_binary()
    brain_path = sandbox.platform.brain_path
    brain_path.parent.mkdir(parents=True, exist_ok=True)

    # Scan
    scanner = Scanner(sandbox.platform)
    found = scanner.scan_found()
    assert len(found) >= 2

    # Configure
    for tool in found:
        configurator = get_configurator(tool)
        if configurator:
            report = configurator.configure(tool, brain_path, amem_binary)
            assert report.result == ConfigResult.SUCCESS

    # Re-scan — should now show ALREADY_CONFIGURED
    tools2 = scanner.scan()
    claude = next(t for t in tools2 if t.tool_id == "claude_code")
    assert claude.status == ToolStatus.ALREADY_CONFIGURED

    cursor = next(t for t in tools2 if t.tool_id == "cursor")
    assert cursor.status == ToolStatus.ALREADY_CONFIGURED

    # Verify
    for tool in [claude, cursor]:
        configurator = get_configurator(tool)
        assert configurator is not None
        assert configurator.verify(tool) is True
