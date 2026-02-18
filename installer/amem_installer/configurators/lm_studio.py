"""LM Studio configurator — similar to Ollama (wrapper-based)."""

from __future__ import annotations

from pathlib import Path
from typing import TYPE_CHECKING

from amem_installer.configurators.base import ConfigReport, ConfigResult, Configurator

if TYPE_CHECKING:
    from amem_installer.scanner import DetectedTool


class LMStudioConfigurator(Configurator):
    """Configures LM Studio integration."""

    def configure(
        self,
        tool: DetectedTool,
        brain_path: Path,
        amem_binary: Path,
        dry_run: bool = False,
    ) -> ConfigReport:
        config_path = brain_path.parent / "lm-studio-amem.yaml"
        config_content = f"""# AgenticMemory LM Studio Integration
brain_path: {brain_path}
amem_binary: {amem_binary}
api_base: http://localhost:1234/v1
"""
        if not dry_run:
            brain_path.parent.mkdir(parents=True, exist_ok=True)
            config_path.write_text(config_content)

        return ConfigReport(
            tool_name=tool.name, tool_id=tool.tool_id,
            result=ConfigResult.SUCCESS,
            config_path=str(config_path), backup_path=None,
            message=f"Config created at {config_path}",
            restart_required=False,
        )

    def unconfigure(
        self,
        tool: DetectedTool,
        backup_path: str | Path | None = None,
    ) -> ConfigReport:
        config = Path.home() / ".amem" / "lm-studio-amem.yaml"
        if config.exists():
            config.unlink()
        return ConfigReport(
            tool_name=tool.name, tool_id=tool.tool_id,
            result=ConfigResult.SUCCESS,
            config_path=None, backup_path=None,
            message="Removed LM Studio config",
            restart_required=False,
        )

    def verify(self, tool: DetectedTool) -> bool:
        return (Path.home() / ".amem" / "lm-studio-amem.yaml").exists()
