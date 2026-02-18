"""Abstract configurator interface and shared types."""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
from pathlib import Path
from typing import TYPE_CHECKING, Optional

if TYPE_CHECKING:
    from amem_installer.scanner import DetectedTool


class ConfigResult(str, Enum):
    """Result of a configuration operation."""
    SUCCESS = "success"
    ALREADY_CONFIGURED = "already_configured"
    SKIPPED = "skipped"
    FAILED = "failed"


@dataclass
class ConfigReport:
    """Result of configuring a single tool.

    Attributes:
        tool_name: Display name of the tool.
        tool_id: Internal tool identifier.
        result: The configuration result.
        config_path: Path that was modified (if any).
        backup_path: Path to backup file (if created).
        message: Human-readable result message.
        restart_required: Whether the tool needs restarting.
    """
    tool_name: str
    tool_id: str
    result: ConfigResult
    config_path: str | None
    backup_path: str | None
    message: str
    restart_required: bool


class Configurator:
    """Abstract base for tool configurators."""

    def configure(
        self,
        tool: DetectedTool,
        brain_path: Path,
        amem_binary: Path,
        dry_run: bool = False,
    ) -> ConfigReport:
        """Configure the tool to use AgenticMemory.

        Args:
            tool: The detected tool to configure.
            brain_path: Path to the shared .amem brain file.
            amem_binary: Path to the amem CLI binary.
            dry_run: If True, show what would be done without doing it.

        Returns:
            ConfigReport with the result.
        """
        raise NotImplementedError

    def unconfigure(
        self,
        tool: DetectedTool,
        backup_path: str | Path | None = None,
    ) -> ConfigReport:
        """Remove AgenticMemory configuration from the tool.

        If backup_path is provided, restore the original config.
        Otherwise, surgically remove only the AgenticMemory entries.

        Args:
            tool: The tool to unconfigure.
            backup_path: Path to backup file (if available).

        Returns:
            ConfigReport with the result.
        """
        raise NotImplementedError

    def verify(self, tool: DetectedTool) -> bool:
        """Verify the tool is correctly configured for AgenticMemory.

        Args:
            tool: The tool to verify.

        Returns:
            True if correctly configured.
        """
        return False
