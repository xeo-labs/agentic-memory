"""LangChain configurator — prints integration instructions."""

from __future__ import annotations

from pathlib import Path
from typing import TYPE_CHECKING

from amem_installer.configurators.base import ConfigReport, ConfigResult, Configurator

if TYPE_CHECKING:
    from amem_installer.scanner import DetectedTool


class LangChainConfigurator(Configurator):
    """Prints LangChain integration instructions (no config modification)."""

    def configure(
        self,
        tool: DetectedTool,
        brain_path: Path,
        amem_binary: Path,
        dry_run: bool = False,
    ) -> ConfigReport:
        instructions = (
            f"To integrate AgenticMemory with your LangChain project, add:\n\n"
            f"    from agentic_memory import Brain\n"
            f'    brain = Brain("{brain_path}")\n\n'
            f"    # Use brain.facts() and brain.search() in your retrieval chain"
        )
        return ConfigReport(
            tool_name=tool.name, tool_id=tool.tool_id,
            result=ConfigResult.SUCCESS,
            config_path=None, backup_path=None,
            message=instructions,
            restart_required=False,
        )

    def unconfigure(self, tool: DetectedTool, backup_path: str | Path | None = None) -> ConfigReport:
        return ConfigReport(
            tool_name=tool.name, tool_id=tool.tool_id,
            result=ConfigResult.SUCCESS,
            config_path=None, backup_path=None,
            message="No config to remove (framework integration is code-level)",
            restart_required=False,
        )
