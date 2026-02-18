"""Tests for configuration loading (amem_agent.config).

Environment variables that might leak from the host are patched out
so tests are reproducible.
"""

from __future__ import annotations

from unittest.mock import patch

import pytest

from amem_agent.config import (
    AgentConfig,
    Config,
    DisplayConfig,
    MemoryConfig,
    _build_agent_config,
    _build_memory_config,
    load_config,
)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

# Environment variables that load_config reads from the environment.  We
# clear them all in every test to prevent host leakage.
_ENV_KEYS = [
    "AMEM_BACKEND",
    "AMEM_MODEL",
    "ANTHROPIC_API_KEY",
    "OPENAI_API_KEY",
    "OLLAMA_URL",
    "OLLAMA_MODEL",
    "AMEM_BRAIN_PATH",
    "AMEM_BINARY",
    "AMEM_VERBOSE",
]


def _clean_env() -> dict[str, str]:
    """Return a dict suitable for ``patch.dict(os.environ, ...)`` that removes
    all env vars that load_config inspects."""
    return {k: "" for k in _ENV_KEYS}


# ---------------------------------------------------------------------------
# Default config
# ---------------------------------------------------------------------------


class TestDefaultConfig:
    @patch.dict("os.environ", {k: "" for k in _ENV_KEYS}, clear=False)
    def test_default_config(self):
        """Loading with no CLI args, no env, no YAML should return built-in defaults."""
        # Pass an empty argv and point config to a nonexistent file.
        cfg = load_config(["--config", "/nonexistent/config.yaml"])

        assert cfg.backend == "anthropic"
        assert cfg.model is None
        assert cfg.verbose is False
        assert isinstance(cfg.memory, MemoryConfig)
        assert cfg.memory.enabled is True
        assert cfg.memory.max_context_tokens == 2000
        assert isinstance(cfg.agent, AgentConfig)
        assert cfg.agent.name == "Amem Agent"
        assert isinstance(cfg.display, DisplayConfig)
        assert cfg.display.color_theme == "default"


# ---------------------------------------------------------------------------
# CLI overrides
# ---------------------------------------------------------------------------


class TestCliArgsOverride:
    @patch.dict("os.environ", {k: "" for k in _ENV_KEYS}, clear=False)
    def test_cli_args_override(self):
        """CLI flags should override defaults."""
        cfg = load_config([
            "--backend", "openai",
            "--model", "gpt-4o",
            "--brain", "/tmp/custom.amem",
            "--verbose",
            "--config", "/nonexistent/config.yaml",
        ])

        assert cfg.backend == "openai"
        assert cfg.model == "gpt-4o"
        assert cfg.brain_path == "/tmp/custom.amem"
        assert cfg.verbose is True


# ---------------------------------------------------------------------------
# --no-memory flag
# ---------------------------------------------------------------------------


class TestNoMemoryFlag:
    @patch.dict("os.environ", {k: "" for k in _ENV_KEYS}, clear=False)
    def test_no_memory_flag(self):
        """--no-memory should disable all memory features."""
        cfg = load_config([
            "--no-memory",
            "--config", "/nonexistent/config.yaml",
        ])

        assert cfg.memory.enabled is False
        assert cfg.memory.extract_events is False
        assert cfg.memory.generate_embeddings is False
        assert cfg.memory.compress_on_exit is False


# ---------------------------------------------------------------------------
# --version flag
# ---------------------------------------------------------------------------


class TestVersionFlag:
    @patch.dict("os.environ", {k: "" for k in _ENV_KEYS}, clear=False)
    def test_version_flag(self):
        """--version should set the _show_version_and_exit attribute."""
        cfg = load_config([
            "--version",
            "--config", "/nonexistent/config.yaml",
        ])

        assert cfg._show_version_and_exit is True


# ---------------------------------------------------------------------------
# --stats flag
# ---------------------------------------------------------------------------


class TestStatsFlag:
    @patch.dict("os.environ", {k: "" for k in _ENV_KEYS}, clear=False)
    def test_stats_flag(self):
        """--stats should set the _show_stats_and_exit attribute."""
        cfg = load_config([
            "--stats",
            "--config", "/nonexistent/config.yaml",
        ])

        assert cfg._show_stats_and_exit is True


# ---------------------------------------------------------------------------
# _build_memory_config
# ---------------------------------------------------------------------------


class TestBuildMemoryConfigDefaults:
    def test_build_memory_config_defaults(self):
        """With empty YAML and no --no-memory, all defaults should hold."""
        mc = _build_memory_config(yaml_section={}, cli_no_memory=False)

        assert mc.enabled is True
        assert mc.max_context_tokens == 2000
        assert mc.extract_events is True
        assert mc.generate_embeddings is True
        assert mc.compress_on_exit is True

    def test_build_memory_config_yaml_override(self):
        """YAML values should override defaults."""
        mc = _build_memory_config(
            yaml_section={
                "enabled": False,
                "max_context_tokens": 4000,
                "extract_events": False,
            },
            cli_no_memory=False,
        )

        assert mc.enabled is False
        assert mc.max_context_tokens == 4000
        assert mc.extract_events is False
        # Non-overridden values keep their defaults.
        assert mc.generate_embeddings is True

    def test_build_memory_config_no_memory_overrides_yaml(self):
        """--no-memory should force-disable even if YAML says enabled."""
        mc = _build_memory_config(
            yaml_section={"enabled": True, "extract_events": True},
            cli_no_memory=True,
        )

        assert mc.enabled is False
        assert mc.extract_events is False
        assert mc.generate_embeddings is False
        assert mc.compress_on_exit is False


# ---------------------------------------------------------------------------
# _build_agent_config
# ---------------------------------------------------------------------------


class TestBuildAgentConfig:
    def test_build_agent_config(self):
        """YAML values should override agent defaults."""
        ac = _build_agent_config({
            "name": "Custom Agent",
            "max_history": 10,
            "system_prompt": "You are helpful.",
        })

        assert ac.name == "Custom Agent"
        assert ac.max_history == 10
        assert ac.system_prompt == "You are helpful."

    def test_build_agent_config_empty(self):
        """Empty YAML section should yield all defaults."""
        ac = _build_agent_config({})

        assert ac.name == "Amem Agent"
        assert ac.max_history == 5
        assert ac.system_prompt is None
