"""Tool detection engine.

Finds every AI tool on the machine without requiring the user to
tell us what's installed. Each detector is a small class that checks
for one specific tool.
"""

from __future__ import annotations

import json
import logging
import shutil
import subprocess
import urllib.request
from dataclasses import dataclass
from enum import Enum
from pathlib import Path
from typing import Optional

from amem_installer.platform import PlatformInfo

logger = logging.getLogger(__name__)


class ToolStatus(str, Enum):
    """Detection status for a tool."""
    FOUND = "found"
    NOT_FOUND = "not_found"
    ALREADY_CONFIGURED = "already_configured"
    UNSUPPORTED_PLATFORM = "unsupported_platform"
    RUNNING = "running"
    NOT_RUNNING = "not_running"


@dataclass
class DetectedTool:
    """A tool detected on the system.

    Attributes:
        name: Display name (e.g., "Claude Code").
        tool_id: Internal ID (e.g., "claude_code").
        status: Detection result.
        config_path: Path to its config file (if applicable).
        version: Version string if detectable.
        integration_type: "mcp", "config", "service", or "framework".
        notes: Additional info (e.g., "3 models available").
    """
    name: str
    tool_id: str
    status: ToolStatus
    config_path: Path | None
    version: str | None
    integration_type: str
    notes: str | None


# ===================================================================
# Base Detector
# ===================================================================

class ToolDetector:
    """Abstract base for tool detection."""
    tool_name: str = ""
    tool_id: str = ""
    integration_type: str = ""

    def __init__(self, platform: PlatformInfo) -> None:
        self.platform = platform

    def detect(self) -> DetectedTool:
        """Check if this tool is present. Returns a DetectedTool."""
        raise NotImplementedError


# ===================================================================
# Tier 1: MCP Tool Detectors
# ===================================================================

class _MCPDetectorBase(ToolDetector):
    """Base class for MCP-based tool detection."""

    def _config_paths(self) -> list[Path]:
        """Return list of possible config file paths."""
        raise NotImplementedError

    def detect(self) -> DetectedTool:
        for path in self._config_paths():
            if path.exists():
                already = self._check_already_configured(path)
                return DetectedTool(
                    name=self.tool_name,
                    tool_id=self.tool_id,
                    status=ToolStatus.ALREADY_CONFIGURED if already else ToolStatus.FOUND,
                    config_path=path,
                    version=None,
                    integration_type=self.integration_type,
                    notes=None,
                )

        return DetectedTool(
            name=self.tool_name, tool_id=self.tool_id,
            status=ToolStatus.NOT_FOUND, config_path=None,
            version=None, integration_type=self.integration_type, notes=None,
        )

    def _check_already_configured(self, path: Path) -> bool:
        """Check if AgenticMemory MCP server is already in the config."""
        try:
            config = json.loads(path.read_text())
            servers = config.get("mcpServers", {})
            return "agentic-memory" in servers or "amem" in servers
        except Exception:
            return False


class ClaudeCodeDetector(_MCPDetectorBase):
    tool_name = "Claude Code"
    tool_id = "claude_code"
    integration_type = "mcp"

    def _config_paths(self) -> list[Path]:
        return [
            self.platform.home / ".claude" / "claude_desktop_config.json",
            self.platform.home / ".claude.json",
        ]


class ClaudeDesktopDetector(_MCPDetectorBase):
    tool_name = "Claude Desktop"
    tool_id = "claude_desktop"
    integration_type = "mcp"

    def _config_paths(self) -> list[Path]:
        if self.platform.os == "darwin":
            return [self.platform.config_dir / "Claude" / "claude_desktop_config.json"]
        elif self.platform.os == "linux":
            return [self.platform.config_dir / "Claude" / "claude_desktop_config.json"]
        else:
            return [self.platform.config_dir / "Claude" / "claude_desktop_config.json"]


class CursorDetector(_MCPDetectorBase):
    tool_name = "Cursor"
    tool_id = "cursor"
    integration_type = "mcp"

    def _config_paths(self) -> list[Path]:
        return [self.platform.home / ".cursor" / "mcp.json"]


class WindsurfDetector(_MCPDetectorBase):
    tool_name = "Windsurf"
    tool_id = "windsurf"
    integration_type = "mcp"

    def _config_paths(self) -> list[Path]:
        return [
            self.platform.home / ".windsurf" / "mcp.json",
            self.platform.home / ".codeium" / "windsurf" / "mcp_config.json",
        ]


# ===================================================================
# Tier 2: Config-File Tool Detectors
# ===================================================================

class ContinueDetector(ToolDetector):
    tool_name = "Continue (VS Code)"
    tool_id = "continue"
    integration_type = "config"

    def detect(self) -> DetectedTool:
        config_path = self.platform.home / ".continue" / "config.json"
        if config_path.exists():
            already = self._check_already_configured(config_path)
            return DetectedTool(
                name=self.tool_name, tool_id=self.tool_id,
                status=ToolStatus.ALREADY_CONFIGURED if already else ToolStatus.FOUND,
                config_path=config_path, version=None,
                integration_type=self.integration_type, notes=None,
            )
        return DetectedTool(
            name=self.tool_name, tool_id=self.tool_id,
            status=ToolStatus.NOT_FOUND, config_path=None,
            version=None, integration_type=self.integration_type, notes=None,
        )

    def _check_already_configured(self, path: Path) -> bool:
        try:
            config = json.loads(path.read_text())
            for cp in config.get("contextProviders", []):
                if cp.get("name") == "agentic-memory":
                    return True
            return False
        except Exception:
            return False


class OpenClawDetector(ToolDetector):
    tool_name = "OpenClaw"
    tool_id = "openclaw"
    integration_type = "config"

    def detect(self) -> DetectedTool:
        # Check binary in PATH
        has_binary = shutil.which("openclaw") is not None

        # Check config directory
        config_path = self.platform.home / ".config" / "openclaw" / "config.yaml"
        if config_path.exists():
            already = self._check_already_configured(config_path)
            return DetectedTool(
                name=self.tool_name, tool_id=self.tool_id,
                status=ToolStatus.ALREADY_CONFIGURED if already else ToolStatus.FOUND,
                config_path=config_path, version=None,
                integration_type=self.integration_type, notes=None,
            )
        elif has_binary:
            return DetectedTool(
                name=self.tool_name, tool_id=self.tool_id,
                status=ToolStatus.FOUND, config_path=config_path,
                version=None, integration_type=self.integration_type,
                notes="Binary found, no config file",
            )
        return DetectedTool(
            name=self.tool_name, tool_id=self.tool_id,
            status=ToolStatus.NOT_FOUND, config_path=None,
            version=None, integration_type=self.integration_type, notes=None,
        )

    def _check_already_configured(self, path: Path) -> bool:
        try:
            import yaml
            config = yaml.safe_load(path.read_text()) or {}
            return config.get("memory", {}).get("provider") == "agentic-memory"
        except Exception:
            return False


# ===================================================================
# Tier 3: Service Detectors
# ===================================================================

class OllamaDetector(ToolDetector):
    tool_name = "Ollama"
    tool_id = "ollama"
    integration_type = "service"

    def detect(self) -> DetectedTool:
        has_binary = shutil.which("ollama") is not None
        is_running = self._check_running()

        if is_running:
            models = self._list_models()
            return DetectedTool(
                name=self.tool_name, tool_id=self.tool_id,
                status=ToolStatus.RUNNING, config_path=None,
                version=self._detect_version(),
                integration_type=self.integration_type,
                notes=f"{len(models)} model(s) available" if models else None,
            )
        elif has_binary:
            return DetectedTool(
                name=self.tool_name, tool_id=self.tool_id,
                status=ToolStatus.NOT_RUNNING, config_path=None,
                version=None, integration_type=self.integration_type,
                notes="Binary found but service not running",
            )
        return DetectedTool(
            name=self.tool_name, tool_id=self.tool_id,
            status=ToolStatus.NOT_FOUND, config_path=None,
            version=None, integration_type=self.integration_type, notes=None,
        )

    def _check_running(self) -> bool:
        try:
            req = urllib.request.urlopen("http://localhost:11434/api/tags", timeout=3)
            return req.status == 200
        except Exception:
            return False

    def _list_models(self) -> list[str]:
        try:
            req = urllib.request.urlopen("http://localhost:11434/api/tags", timeout=3)
            data = json.loads(req.read())
            return [m["name"] for m in data.get("models", [])]
        except Exception:
            return []

    def _detect_version(self) -> str | None:
        try:
            result = subprocess.run(
                ["ollama", "--version"], capture_output=True, text=True, timeout=5,
            )
            return result.stdout.strip() if result.returncode == 0 else None
        except Exception:
            return None


class LMStudioDetector(ToolDetector):
    tool_name = "LM Studio"
    tool_id = "lm_studio"
    integration_type = "service"

    def detect(self) -> DetectedTool:
        is_running = self._check_running()
        if is_running:
            return DetectedTool(
                name=self.tool_name, tool_id=self.tool_id,
                status=ToolStatus.RUNNING, config_path=None,
                version=None, integration_type=self.integration_type, notes=None,
            )
        return DetectedTool(
            name=self.tool_name, tool_id=self.tool_id,
            status=ToolStatus.NOT_FOUND, config_path=None,
            version=None, integration_type=self.integration_type, notes=None,
        )

    def _check_running(self) -> bool:
        try:
            req = urllib.request.urlopen("http://localhost:1234/v1/models", timeout=3)
            return req.status == 200
        except Exception:
            return False


# ===================================================================
# Tier 4: Framework Detectors
# ===================================================================

class _FrameworkDetector(ToolDetector):
    """Base for Python framework detection in current project."""
    _package_names: list[str] = []

    def detect(self) -> DetectedTool:
        cwd = Path.cwd()

        # Check requirements.txt
        req = cwd / "requirements.txt"
        if req.exists():
            text = req.read_text().lower()
            for pkg in self._package_names:
                if pkg in text:
                    return DetectedTool(
                        name=self.tool_name, tool_id=self.tool_id,
                        status=ToolStatus.FOUND, config_path=req,
                        version=None, integration_type=self.integration_type,
                        notes="Found in requirements.txt",
                    )

        # Check pyproject.toml
        pyp = cwd / "pyproject.toml"
        if pyp.exists():
            text = pyp.read_text().lower()
            for pkg in self._package_names:
                if pkg in text:
                    return DetectedTool(
                        name=self.tool_name, tool_id=self.tool_id,
                        status=ToolStatus.FOUND, config_path=pyp,
                        version=None, integration_type=self.integration_type,
                        notes="Found in pyproject.toml",
                    )

        return DetectedTool(
            name=self.tool_name, tool_id=self.tool_id,
            status=ToolStatus.NOT_FOUND, config_path=None,
            version=None, integration_type=self.integration_type, notes=None,
        )


class LangChainDetector(_FrameworkDetector):
    tool_name = "LangChain"
    tool_id = "langchain"
    integration_type = "framework"
    _package_names = ["langchain"]


class CrewAIDetector(_FrameworkDetector):
    tool_name = "CrewAI"
    tool_id = "crewai"
    integration_type = "framework"
    _package_names = ["crewai"]


class AutoGenDetector(_FrameworkDetector):
    tool_name = "AutoGen"
    tool_id = "autogen"
    integration_type = "framework"
    _package_names = ["autogen", "pyautogen"]


# ===================================================================
# Scanner
# ===================================================================

class Scanner:
    """Detects installed AI tools on the system."""

    def __init__(self, platform_info: PlatformInfo) -> None:
        self.platform = platform_info
        self._detectors: list[ToolDetector] = [
            # Tier 1: MCP tools
            ClaudeCodeDetector(platform_info),
            ClaudeDesktopDetector(platform_info),
            CursorDetector(platform_info),
            WindsurfDetector(platform_info),
            # Tier 2: Config tools
            ContinueDetector(platform_info),
            OpenClawDetector(platform_info),
            # Tier 3: Services
            OllamaDetector(platform_info),
            LMStudioDetector(platform_info),
            # Tier 4: Frameworks
            LangChainDetector(platform_info),
            CrewAIDetector(platform_info),
            AutoGenDetector(platform_info),
        ]

    def scan(self) -> list[DetectedTool]:
        """Scan for all supported tools.

        Returns:
            List of DetectedTool, one per supported tool (even if not found).
        """
        results: list[DetectedTool] = []
        for detector in self._detectors:
            try:
                result = detector.detect()
                results.append(result)
            except Exception as e:
                logger.debug("Detection error for %s: %s", detector.tool_name, e)
                results.append(DetectedTool(
                    name=detector.tool_name,
                    tool_id=detector.tool_id,
                    status=ToolStatus.NOT_FOUND,
                    config_path=None,
                    version=None,
                    integration_type=detector.integration_type,
                    notes=f"Detection error: {e}",
                ))
        return results

    def scan_found(self) -> list[DetectedTool]:
        """Scan and return only tools that were found/running/configured."""
        return [
            t for t in self.scan()
            if t.status in (ToolStatus.FOUND, ToolStatus.RUNNING, ToolStatus.ALREADY_CONFIGURED)
        ]
