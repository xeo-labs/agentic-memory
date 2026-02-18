"""Configurator factory — maps tool IDs to configurator instances."""

from __future__ import annotations

from typing import TYPE_CHECKING, Optional

from amem_installer.configurators.base import Configurator, ConfigReport, ConfigResult
from amem_installer.configurators.mcp import MCPConfigurator
from amem_installer.configurators.continue_ext import ContinueConfigurator
from amem_installer.configurators.ollama import OllamaConfigurator
from amem_installer.configurators.openclaw import OpenClawConfigurator
from amem_installer.configurators.lm_studio import LMStudioConfigurator
from amem_installer.configurators.langchain import LangChainConfigurator
from amem_installer.configurators.crewai import CrewAIConfigurator
from amem_installer.configurators.autogen import AutoGenConfigurator

if TYPE_CHECKING:
    from amem_installer.scanner import DetectedTool

__all__ = [
    "get_configurator",
    "Configurator",
    "ConfigReport",
    "ConfigResult",
    "MCPConfigurator",
    "ContinueConfigurator",
    "OllamaConfigurator",
    "OpenClawConfigurator",
    "LMStudioConfigurator",
    "LangChainConfigurator",
    "CrewAIConfigurator",
    "AutoGenConfigurator",
]


def get_configurator(tool: DetectedTool) -> Configurator | None:
    """Get the appropriate configurator for a detected tool.

    Args:
        tool: The detected tool to get a configurator for.

    Returns:
        A Configurator instance, or None if no configurator exists for this tool.
    """
    configurator_map: dict[str, Configurator] = {
        # MCP tools
        "claude_code": MCPConfigurator(),
        "claude_desktop": MCPConfigurator(),
        "cursor": MCPConfigurator(),
        "windsurf": MCPConfigurator(),
        # Config tools
        "continue": ContinueConfigurator(),
        "openclaw": OpenClawConfigurator(),
        # Services
        "ollama": OllamaConfigurator(),
        "lm_studio": LMStudioConfigurator(),
        # Frameworks (instructions only)
        "langchain": LangChainConfigurator(),
        "crewai": CrewAIConfigurator(),
        "autogen": AutoGenConfigurator(),
    }
    return configurator_map.get(tool.tool_id)
