"""Configuration loading for amem-agent.

Loads settings from (highest to lowest priority):
    1. CLI arguments
    2. Environment variables
    3. YAML config file
    4. Defaults
"""

from __future__ import annotations

import argparse
import logging
import os
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import yaml

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Default paths
# ---------------------------------------------------------------------------

DEFAULT_CONFIG_DIR = Path.home() / ".amem"
DEFAULT_BRAIN_PATH = str(DEFAULT_CONFIG_DIR / "brain.amem")
DEFAULT_CONFIG_PATH = str(DEFAULT_CONFIG_DIR / "config.yaml")
DEFAULT_AMEM_BINARY = "amem"


# ---------------------------------------------------------------------------
# Configuration error
# ---------------------------------------------------------------------------

class ConfigError(Exception):
    """Raised when configuration is invalid or incomplete."""


# ---------------------------------------------------------------------------
# Nested configuration dataclasses
# ---------------------------------------------------------------------------

@dataclass
class MemoryConfig:
    """Settings that control how memory formation works."""

    enabled: bool = True
    max_context_tokens: int = 2000
    extract_events: bool = True
    generate_embeddings: bool = True
    compress_on_exit: bool = True


@dataclass
class AgentConfig:
    """Settings that control the agent's personality and behaviour."""

    name: str = "Amem Agent"
    max_history: int = 5
    system_prompt: str | None = None


@dataclass
class DisplayConfig:
    """Settings that control terminal display."""

    show_memory_stats: bool = True
    show_extraction: bool = False
    color_theme: str = "default"


# ---------------------------------------------------------------------------
# Top-level configuration
# ---------------------------------------------------------------------------

@dataclass
class Config:
    """Complete application configuration.

    Constructed via :func:`load_config`, which merges CLI arguments,
    environment variables, a YAML config file, and built-in defaults.
    """

    # LLM backend
    backend: str = "anthropic"
    model: str | None = None

    # API keys
    anthropic_api_key: str | None = None
    openai_api_key: str | None = None

    # Ollama
    ollama_url: str = "http://localhost:11434"
    ollama_model: str = "llama3.2"

    # Brain / amem CLI
    brain_path: str = DEFAULT_BRAIN_PATH
    amem_binary: str = DEFAULT_AMEM_BINARY

    # General
    verbose: bool = False

    # Nested sections
    memory: MemoryConfig = field(default_factory=MemoryConfig)
    agent: AgentConfig = field(default_factory=AgentConfig)
    display: DisplayConfig = field(default_factory=DisplayConfig)


# ---------------------------------------------------------------------------
# CLI argument parsing
# ---------------------------------------------------------------------------

def _build_parser() -> argparse.ArgumentParser:
    """Build the CLI argument parser.

    Returns:
        A configured :class:`argparse.ArgumentParser`.
    """
    parser = argparse.ArgumentParser(
        prog="amem-agent",
        description="Terminal AI agent with persistent AgenticMemory",
    )
    parser.add_argument(
        "--backend",
        choices=["anthropic", "openai", "ollama"],
        help="LLM backend to use (default: from config)",
    )
    parser.add_argument(
        "--model",
        help="Model name override (default: from config)",
    )
    parser.add_argument(
        "--brain",
        dest="brain_path",
        help=f"Path to .amem file (default: {DEFAULT_BRAIN_PATH})",
    )
    parser.add_argument(
        "--config",
        dest="config_path",
        default=None,
        help=f"Path to config.yaml (default: {DEFAULT_CONFIG_PATH})",
    )
    parser.add_argument(
        "--session-id",
        type=int,
        default=None,
        help="Force a specific session ID (default: auto-increment)",
    )
    parser.add_argument(
        "--verbose",
        action="store_true",
        default=None,
        help="Enable debug logging",
    )
    parser.add_argument(
        "--no-memory",
        action="store_true",
        default=False,
        help="Disable memory (for comparison testing)",
    )
    parser.add_argument(
        "--stats",
        action="store_true",
        default=False,
        help="Show brain stats and exit",
    )
    parser.add_argument(
        "--version",
        action="store_true",
        default=False,
        help="Show version and exit",
    )
    return parser


# ---------------------------------------------------------------------------
# YAML loading
# ---------------------------------------------------------------------------

def _load_yaml(path: str | None) -> dict[str, Any]:
    """Load a YAML config file, returning an empty dict on any failure.

    Args:
        path: Filesystem path to the YAML file, or ``None`` to skip.

    Returns:
        Parsed YAML as a dictionary, or an empty dictionary if the file does
        not exist or cannot be parsed.
    """
    if path is None:
        return {}

    config_path = Path(path).expanduser()
    if not config_path.is_file():
        logger.debug("Config file not found: %s", config_path)
        return {}

    try:
        with open(config_path, "r", encoding="utf-8") as fh:
            data = yaml.safe_load(fh)
            return data if isinstance(data, dict) else {}
    except yaml.YAMLError as exc:
        logger.warning("Failed to parse config file %s: %s", config_path, exc)
        return {}
    except OSError as exc:
        logger.warning("Failed to read config file %s: %s", config_path, exc)
        return {}


# ---------------------------------------------------------------------------
# Nested-section helpers
# ---------------------------------------------------------------------------

def _build_memory_config(
    yaml_section: dict[str, Any],
    cli_no_memory: bool,
) -> MemoryConfig:
    """Build a :class:`MemoryConfig` from YAML values and CLI overrides.

    Args:
        yaml_section: The ``memory`` section of the YAML config (may be empty).
        cli_no_memory: If ``True``, disables memory entirely.

    Returns:
        A populated :class:`MemoryConfig`.
    """
    cfg = MemoryConfig(
        enabled=yaml_section.get("enabled", MemoryConfig.enabled),
        max_context_tokens=yaml_section.get(
            "max_context_tokens", MemoryConfig.max_context_tokens
        ),
        extract_events=yaml_section.get(
            "extract_events", MemoryConfig.extract_events
        ),
        generate_embeddings=yaml_section.get(
            "generate_embeddings", MemoryConfig.generate_embeddings
        ),
        compress_on_exit=yaml_section.get(
            "compress_on_exit", MemoryConfig.compress_on_exit
        ),
    )
    if cli_no_memory:
        cfg.enabled = False
        cfg.extract_events = False
        cfg.generate_embeddings = False
        cfg.compress_on_exit = False
    return cfg


def _build_agent_config(yaml_section: dict[str, Any]) -> AgentConfig:
    """Build an :class:`AgentConfig` from YAML values.

    Args:
        yaml_section: The ``agent`` section of the YAML config (may be empty).

    Returns:
        A populated :class:`AgentConfig`.
    """
    return AgentConfig(
        name=yaml_section.get("name", AgentConfig.name),
        max_history=yaml_section.get("max_history", AgentConfig.max_history),
        system_prompt=yaml_section.get("system_prompt", AgentConfig.system_prompt),
    )


def _build_display_config(yaml_section: dict[str, Any]) -> DisplayConfig:
    """Build a :class:`DisplayConfig` from YAML values.

    Args:
        yaml_section: The ``display`` section of the YAML config (may be empty).

    Returns:
        A populated :class:`DisplayConfig`.
    """
    return DisplayConfig(
        show_memory_stats=yaml_section.get(
            "show_memory_stats", DisplayConfig.show_memory_stats
        ),
        show_extraction=yaml_section.get(
            "show_extraction", DisplayConfig.show_extraction
        ),
        color_theme=yaml_section.get("color_theme", DisplayConfig.color_theme),
    )


# ---------------------------------------------------------------------------
# Public API
# ---------------------------------------------------------------------------

def load_config(argv: list[str] | None = None) -> Config:
    """Load configuration by merging all sources.

    Priority (highest first):
        1. CLI arguments (from *argv* or ``sys.argv``)
        2. Environment variables
        3. YAML config file
        4. Built-in defaults

    Args:
        argv: Explicit argument list. Pass ``None`` to read from ``sys.argv``.

    Returns:
        A fully-resolved :class:`Config` instance.

    Raises:
        ConfigError: If the resulting configuration is not usable (e.g. no
            API key for the chosen backend).
    """
    # ---- 1. Parse CLI arguments ----
    parser = _build_parser()
    args = parser.parse_args(argv)

    # ---- 2. Load YAML ----
    config_path = args.config_path or DEFAULT_CONFIG_PATH
    yaml_data = _load_yaml(config_path)

    # ---- 3. Resolve each field: CLI -> env -> YAML -> default ----

    backend = (
        args.backend
        or os.environ.get("AMEM_BACKEND")
        or yaml_data.get("backend")
        or Config.backend
    )

    model = (
        args.model
        or os.environ.get("AMEM_MODEL")
        or yaml_data.get("model")
        or Config.model
    )

    anthropic_api_key = (
        os.environ.get("ANTHROPIC_API_KEY")
        or yaml_data.get("anthropic_api_key")
        or Config.anthropic_api_key
    )

    openai_api_key = (
        os.environ.get("OPENAI_API_KEY")
        or yaml_data.get("openai_api_key")
        or Config.openai_api_key
    )

    ollama_url = (
        os.environ.get("OLLAMA_URL")
        or yaml_data.get("ollama_url")
        or Config.ollama_url
    )

    ollama_model = (
        os.environ.get("OLLAMA_MODEL")
        or yaml_data.get("ollama_model")
        or Config.ollama_model
    )

    brain_path = (
        args.brain_path
        or os.environ.get("AMEM_BRAIN_PATH")
        or yaml_data.get("brain_path")
        or Config.brain_path
    )
    # Expand ~ in brain path
    brain_path = str(Path(brain_path).expanduser())

    amem_binary = (
        os.environ.get("AMEM_BINARY")
        or yaml_data.get("amem_binary")
        or Config.amem_binary
    )

    verbose: bool
    if args.verbose is not None:
        verbose = args.verbose
    else:
        env_verbose = os.environ.get("AMEM_VERBOSE", "").lower()
        if env_verbose in ("1", "true", "yes"):
            verbose = True
        else:
            verbose = yaml_data.get("verbose", Config.verbose)

    # ---- 4. Build nested configs ----
    memory_yaml = yaml_data.get("memory", {})
    if not isinstance(memory_yaml, dict):
        memory_yaml = {}
    memory = _build_memory_config(memory_yaml, cli_no_memory=args.no_memory)

    agent_yaml = yaml_data.get("agent", {})
    if not isinstance(agent_yaml, dict):
        agent_yaml = {}
    agent = _build_agent_config(agent_yaml)

    display_yaml = yaml_data.get("display", {})
    if not isinstance(display_yaml, dict):
        display_yaml = {}
    display = _build_display_config(display_yaml)

    # ---- 5. Assemble and return ----
    config = Config(
        backend=backend,
        model=model,
        anthropic_api_key=anthropic_api_key,
        openai_api_key=openai_api_key,
        ollama_url=ollama_url,
        ollama_model=ollama_model,
        brain_path=brain_path,
        amem_binary=amem_binary,
        verbose=verbose,
        memory=memory,
        agent=agent,
        display=display,
    )

    # Attach extra CLI-only flags for the entry point to inspect.
    # These are not part of the persisted config but influence startup.
    config._session_id_override = args.session_id  # type: ignore[attr-defined]
    config._show_stats_and_exit = args.stats  # type: ignore[attr-defined]
    config._show_version_and_exit = args.version  # type: ignore[attr-defined]

    return config
