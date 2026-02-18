"""Continue (VS Code extension) configurator."""

from __future__ import annotations

import json
from pathlib import Path
from typing import TYPE_CHECKING

from amem_installer.backup import backup_config
from amem_installer.configurators.base import ConfigReport, ConfigResult, Configurator

if TYPE_CHECKING:
    from amem_installer.scanner import DetectedTool


class ContinueConfigurator(Configurator):
    """Configures the Continue VS Code extension with AgenticMemory."""

    def configure(
        self,
        tool: DetectedTool,
        brain_path: Path,
        amem_binary: Path,
        dry_run: bool = False,
    ) -> ConfigReport:
        config_path = tool.config_path
        if config_path is None or not config_path.exists():
            return ConfigReport(
                tool_name=tool.name, tool_id=tool.tool_id,
                result=ConfigResult.FAILED,
                config_path=None, backup_path=None,
                message="Config file not found",
                restart_required=False,
            )

        config = json.loads(config_path.read_text())

        # Check if already configured
        for cp in config.get("contextProviders", []):
            if cp.get("name") == "agentic-memory":
                return ConfigReport(
                    tool_name=tool.name, tool_id=tool.tool_id,
                    result=ConfigResult.ALREADY_CONFIGURED,
                    config_path=str(config_path), backup_path=None,
                    message="Already configured", restart_required=False,
                )

        # Backup
        backup_p: Path | None = None
        if not dry_run:
            backup_p = backup_config(config_path)

        # Add context provider
        amem_provider = {
            "name": "agentic-memory",
            "params": {
                "brainPath": str(brain_path),
                "amemBinary": str(amem_binary),
            },
        }
        config.setdefault("contextProviders", []).append(amem_provider)

        if not dry_run:
            config_path.write_text(json.dumps(config, indent=2) + "\n")

        return ConfigReport(
            tool_name=tool.name, tool_id=tool.tool_id,
            result=ConfigResult.SUCCESS,
            config_path=str(config_path),
            backup_path=str(backup_p) if backup_p else None,
            message="Context provider added to Continue",
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
                message=f"Restored {tool.name} config from backup",
                restart_required=True,
            )
        elif tool.config_path and tool.config_path.exists():
            config = json.loads(tool.config_path.read_text())
            providers = config.get("contextProviders", [])
            config["contextProviders"] = [
                p for p in providers if p.get("name") != "agentic-memory"
            ]
            tool.config_path.write_text(json.dumps(config, indent=2) + "\n")
            return ConfigReport(
                tool_name=tool.name, tool_id=tool.tool_id,
                result=ConfigResult.SUCCESS,
                config_path=str(tool.config_path), backup_path=None,
                message="Removed AgenticMemory from Continue",
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
            for cp in config.get("contextProviders", []):
                if cp.get("name") == "agentic-memory":
                    return True
            return False
        except Exception:
            return False
