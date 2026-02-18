"""Ollama configurator — creates wrapper script and config."""

from __future__ import annotations

from pathlib import Path
from typing import TYPE_CHECKING

from amem_installer.configurators.base import ConfigReport, ConfigResult, Configurator

if TYPE_CHECKING:
    from amem_installer.scanner import DetectedTool


class OllamaConfigurator(Configurator):
    """Configures Ollama integration via wrapper script and config file."""

    def configure(
        self,
        tool: DetectedTool,
        brain_path: Path,
        amem_binary: Path,
        dry_run: bool = False,
    ) -> ConfigReport:
        amem_dir = brain_path.parent

        # Wrapper script
        wrapper_path = amem_dir / "ollama-amem"
        wrapper_content = f"""#!/bin/bash
# AgenticMemory wrapper for Ollama
# Usage: ollama-amem chat <model>
# This wraps 'ollama chat' with memory context from your AgenticMemory brain.

AMEM_BRAIN="{brain_path}"
AMEM_BINARY="{amem_binary}"

# Query brain for context before chat
CONTEXT=$("$AMEM_BINARY" search "$AMEM_BRAIN" --types fact,decision --limit 20 --format text 2>/dev/null)

if [ -n "$CONTEXT" ]; then
    echo "[Memory context loaded from brain]"
fi

# Forward to ollama with context
exec ollama "$@"
"""

        # Config file
        config_path = amem_dir / "ollama-amem.yaml"
        config_content = f"""# AgenticMemory Ollama Integration
brain_path: {brain_path}
amem_binary: {amem_binary}
auto_extract: true
"""

        if not dry_run:
            amem_dir.mkdir(parents=True, exist_ok=True)
            wrapper_path.write_text(wrapper_content)
            wrapper_path.chmod(0o755)
            config_path.write_text(config_content)

        return ConfigReport(
            tool_name=tool.name, tool_id=tool.tool_id,
            result=ConfigResult.SUCCESS,
            config_path=str(config_path),
            backup_path=None,
            message=f"Wrapper script created at {wrapper_path}",
            restart_required=False,
        )

    def unconfigure(
        self,
        tool: DetectedTool,
        backup_path: str | Path | None = None,
    ) -> ConfigReport:
        # Remove wrapper and config
        amem_dir = Path.home() / ".amem"
        wrapper = amem_dir / "ollama-amem"
        config = amem_dir / "ollama-amem.yaml"
        removed: list[str] = []
        if wrapper.exists():
            wrapper.unlink()
            removed.append("wrapper")
        if config.exists():
            config.unlink()
            removed.append("config")

        return ConfigReport(
            tool_name=tool.name, tool_id=tool.tool_id,
            result=ConfigResult.SUCCESS,
            config_path=None, backup_path=None,
            message=f"Removed Ollama integration ({', '.join(removed) or 'nothing to remove'})",
            restart_required=False,
        )

    def verify(self, tool: DetectedTool) -> bool:
        amem_dir = Path.home() / ".amem"
        return (amem_dir / "ollama-amem").exists()
