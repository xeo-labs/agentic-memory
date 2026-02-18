"""OpenClaw configurator — YAML-based config."""

from __future__ import annotations

from pathlib import Path
from typing import TYPE_CHECKING

from amem_installer.backup import backup_config
from amem_installer.configurators.base import ConfigReport, ConfigResult, Configurator

if TYPE_CHECKING:
    from amem_installer.scanner import DetectedTool


class OpenClawConfigurator(Configurator):
    """Configures OpenClaw with AgenticMemory as a memory provider."""

    def configure(
        self,
        tool: DetectedTool,
        brain_path: Path,
        amem_binary: Path,
        dry_run: bool = False,
    ) -> ConfigReport:
        import yaml

        config_path = tool.config_path
        if config_path is None:
            return ConfigReport(
                tool_name=tool.name, tool_id=tool.tool_id,
                result=ConfigResult.FAILED,
                config_path=None, backup_path=None,
                message="No config path", restart_required=False,
            )

        if config_path.exists():
            config = yaml.safe_load(config_path.read_text()) or {}
        else:
            config = {}

        # Check if already configured
        memory = config.get("memory", {})
        if memory.get("provider") == "agentic-memory":
            return ConfigReport(
                tool_name=tool.name, tool_id=tool.tool_id,
                result=ConfigResult.ALREADY_CONFIGURED,
                config_path=str(config_path), backup_path=None,
                message="Already configured", restart_required=False,
            )

        # Backup
        backup_p: Path | None = None
        if not dry_run and config_path.exists():
            backup_p = backup_config(config_path)

        # Add memory provider config
        config["memory"] = {
            "provider": "agentic-memory",
            "brain_path": str(brain_path),
            "amem_binary": str(amem_binary),
        }

        if not dry_run:
            config_path.parent.mkdir(parents=True, exist_ok=True)
            config_path.write_text(yaml.dump(config, default_flow_style=False))

        return ConfigReport(
            tool_name=tool.name, tool_id=tool.tool_id,
            result=ConfigResult.SUCCESS,
            config_path=str(config_path),
            backup_path=str(backup_p) if backup_p else None,
            message="Memory provider set to agentic-memory",
            restart_required=True,
        )

    def unconfigure(
        self,
        tool: DetectedTool,
        backup_path: str | Path | None = None,
    ) -> ConfigReport:
        if backup_path and Path(backup_path).exists():
            import shutil
            shutil.copy2(str(backup_path), str(tool.config_path))
            return ConfigReport(
                tool_name=tool.name, tool_id=tool.tool_id,
                result=ConfigResult.SUCCESS,
                config_path=str(tool.config_path),
                backup_path=str(backup_path),
                message="Restored from backup",
                restart_required=True,
            )
        elif tool.config_path and tool.config_path.exists():
            import yaml
            config = yaml.safe_load(tool.config_path.read_text()) or {}
            if "memory" in config:
                del config["memory"]
                tool.config_path.write_text(yaml.dump(config, default_flow_style=False))
            return ConfigReport(
                tool_name=tool.name, tool_id=tool.tool_id,
                result=ConfigResult.SUCCESS,
                config_path=str(tool.config_path), backup_path=None,
                message="Removed memory provider",
                restart_required=True,
            )
        return ConfigReport(
            tool_name=tool.name, tool_id=tool.tool_id,
            result=ConfigResult.FAILED,
            config_path=None, backup_path=None,
            message="No config file", restart_required=False,
        )

    def verify(self, tool: DetectedTool) -> bool:
        if tool.config_path is None or not tool.config_path.exists():
            return False
        try:
            import yaml
            config = yaml.safe_load(tool.config_path.read_text()) or {}
            return config.get("memory", {}).get("provider") == "agentic-memory"
        except Exception:
            return False
