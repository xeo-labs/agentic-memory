"""MCP configurator — handles Claude Code, Cursor, Windsurf, Claude Desktop.

All MCP-based tools use the same JSON format for server configuration.
"""

from __future__ import annotations

import json
import shutil
from pathlib import Path
from typing import TYPE_CHECKING

from amem_installer.backup import backup_config
from amem_installer.configurators.base import ConfigReport, ConfigResult, Configurator

if TYPE_CHECKING:
    from amem_installer.scanner import DetectedTool


class MCPConfigurator(Configurator):
    """Configures MCP-based tools (Claude Code, Cursor, Windsurf, etc.)."""

    def configure(
        self,
        tool: DetectedTool,
        brain_path: Path,
        amem_binary: Path,
        dry_run: bool = False,
    ) -> ConfigReport:
        config_path = tool.config_path
        if config_path is None:
            return ConfigReport(
                tool_name=tool.name, tool_id=tool.tool_id,
                result=ConfigResult.FAILED,
                config_path=None, backup_path=None,
                message="No config path available",
                restart_required=False,
            )

        # Read existing config (or create empty)
        if config_path.exists():
            try:
                config = json.loads(config_path.read_text())
            except json.JSONDecodeError:
                config = {}
        else:
            config = {}

        # Check if already configured
        servers = config.get("mcpServers", {})
        if "agentic-memory" in servers:
            return ConfigReport(
                tool_name=tool.name, tool_id=tool.tool_id,
                result=ConfigResult.ALREADY_CONFIGURED,
                config_path=str(config_path), backup_path=None,
                message=f"{tool.name} already has AgenticMemory configured",
                restart_required=False,
            )

        # Backup existing config
        backup_p: Path | None = None
        if not dry_run and config_path.exists():
            backup_p = backup_config(config_path)

        # Add MCP server entry
        mcp_entry = self._build_mcp_entry(brain_path, amem_binary)
        if "mcpServers" not in config:
            config["mcpServers"] = {}
        config["mcpServers"]["agentic-memory"] = mcp_entry

        # Write config
        if not dry_run:
            config_path.parent.mkdir(parents=True, exist_ok=True)
            config_path.write_text(json.dumps(config, indent=2) + "\n")

        return ConfigReport(
            tool_name=tool.name, tool_id=tool.tool_id,
            result=ConfigResult.SUCCESS,
            config_path=str(config_path),
            backup_path=str(backup_p) if backup_p else None,
            message=f"MCP server configured for {tool.name}",
            restart_required=True,
        )

    def _build_mcp_entry(self, brain_path: Path, amem_binary: Path) -> dict:  # type: ignore[type-arg]
        """Build the MCP server configuration entry."""
        return {
            "command": str(amem_binary),
            "args": ["mcp-serve", "--brain", str(brain_path)],
            "env": {
                "AMEM_BRAIN_PATH": str(brain_path),
            },
        }

    def unconfigure(
        self,
        tool: DetectedTool,
        backup_path: str | Path | None = None,
    ) -> ConfigReport:
        if backup_path and Path(backup_path).exists():
            shutil.copy2(str(backup_path), str(tool.config_path))
            return ConfigReport(
                tool_name=tool.name, tool_id=tool.tool_id,
                result=ConfigResult.SUCCESS,
                config_path=str(tool.config_path),
                backup_path=str(backup_path),
                message=f"Restored {tool.name} config from backup",
                restart_required=True,
            )
        elif tool.config_path and tool.config_path.exists():
            config = json.loads(tool.config_path.read_text())
            servers = config.get("mcpServers", {})
            if "agentic-memory" in servers:
                del servers["agentic-memory"]
                config["mcpServers"] = servers
                tool.config_path.write_text(json.dumps(config, indent=2) + "\n")
            return ConfigReport(
                tool_name=tool.name, tool_id=tool.tool_id,
                result=ConfigResult.SUCCESS,
                config_path=str(tool.config_path), backup_path=None,
                message=f"Removed AgenticMemory from {tool.name}",
                restart_required=True,
            )
        return ConfigReport(
            tool_name=tool.name, tool_id=tool.tool_id,
            result=ConfigResult.FAILED,
            config_path=None, backup_path=None,
            message="No config file to unconfigure",
            restart_required=False,
        )

    def verify(self, tool: DetectedTool) -> bool:
        if tool.config_path is None or not tool.config_path.exists():
            return False
        try:
            config = json.loads(tool.config_path.read_text())
            return "agentic-memory" in config.get("mcpServers", {})
        except Exception:
            return False
