"""
Entry point for the amem-agent CLI.

Run with::

    python -m amem_agent          # uses default settings
    amem-agent --backend openai   # if installed via pip/pipx

Configuration priority (highest to lowest):

1. CLI arguments
2. Environment variables (``ANTHROPIC_API_KEY``, ``OPENAI_API_KEY``, etc.)
3. YAML configuration file
4. Built-in defaults

CLI parsing and config merging are handled by :func:`amem_agent.config.load_config`.
This module focuses on wiring the components together and starting the loop.
"""

from __future__ import annotations

import logging
import os
import sys

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _check_api_key(config: object) -> bool:
    """Verify that the required API key is available for the chosen backend.

    Checks both the config object and common environment variables.  If a
    key is found in the environment but not in config, it is written back
    to config so that downstream components can find it.

    Args:
        config: The resolved configuration object.

    Returns:
        ``True`` if the key is available, ``False`` otherwise.
    """
    backend = getattr(config, "backend", "anthropic")

    if backend == "anthropic":
        key = (
            getattr(config, "anthropic_api_key", None)
            or os.environ.get("ANTHROPIC_API_KEY")
        )
        if not key:
            print(
                "\n  Error: Anthropic API key not found.\n"
                "  Set ANTHROPIC_API_KEY in your environment or config file.\n"
            )
            return False
        config.anthropic_api_key = key

    elif backend == "openai":
        key = (
            getattr(config, "openai_api_key", None)
            or os.environ.get("OPENAI_API_KEY")
        )
        if not key:
            print(
                "\n  Error: OpenAI API key not found.\n"
                "  Set OPENAI_API_KEY in your environment or config file.\n"
            )
            return False
        config.openai_api_key = key

    elif backend == "ollama":
        # Ollama is local -- no API key needed, but ensure URL is set.
        if not getattr(config, "ollama_url", None):
            config.ollama_url = os.environ.get(
                "OLLAMA_URL", "http://localhost:11434"
            )

    return True


def _print_brain_stats(brain: object) -> None:
    """Display brain statistics and exit.

    Args:
        brain: An initialised Brain instance.
    """
    try:
        from amem_agent.utils.display import display_brain_stats

        stats = brain.stats()
        display_brain_stats(stats)
    except Exception as exc:  # noqa: BLE001
        print(f"\n  Error reading brain stats: {exc}\n")


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main() -> None:
    """Parse arguments, initialise components, and start the agent loop.

    This is the primary entry point invoked by ``python -m amem_agent``
    or the ``amem-agent`` console script defined in ``pyproject.toml``.
    """
    # --- Load configuration (handles its own CLI parsing) ---
    from amem_agent.config import load_config

    try:
        config = load_config()  # reads sys.argv internally
    except Exception as exc:  # noqa: BLE001
        print(f"\n  Error loading configuration: {exc}\n")
        sys.exit(1)

    # --- Early-exit flags ---
    if getattr(config, "_show_version_and_exit", False):
        from amem_agent import __version__

        print(f"amem-agent {__version__}")
        return

    # --- Logging ---
    from amem_agent.utils.logger import setup_logging

    setup_logging(verbose=config.verbose)

    # --- API key check ---
    if not _check_api_key(config):
        sys.exit(1)

    # --- Initialise Brain ---
    from amem_agent.brain import Brain

    try:
        brain = Brain(
            brain_path=config.brain_path,
            amem_binary=getattr(config, "amem_binary", "amem"),
        )
        brain.ensure_exists()
    except Exception as exc:  # noqa: BLE001
        print(f"\n  Error initialising brain: {exc}\n")
        logger.exception("Brain initialisation failed")
        sys.exit(1)

    # --- Stats-only mode ---
    if getattr(config, "_show_stats_and_exit", False):
        _print_brain_stats(brain)
        return

    # --- Initialise LLM backend ---
    from amem_agent.llm import create_backend

    try:
        llm = create_backend(config)
    except Exception as exc:  # noqa: BLE001
        print(f"\n  Error initialising LLM backend: {exc}\n")
        logger.exception("LLM backend initialisation failed")
        sys.exit(1)

    # --- Session management ---
    from amem_agent.agent.session import SessionManager

    session_mgr = SessionManager(config.brain_path)

    session_id_override = getattr(config, "_session_id_override", None)
    if session_id_override is not None:
        session_id = session_id_override
    else:
        session_id = session_mgr.next_session_id()

    # --- Create and run the agent loop ---
    from amem_agent.agent.loop import AgentLoop

    loop = AgentLoop(
        brain=brain,
        llm=llm,
        config=config,
        session_id=session_id,
    )

    try:
        loop.run()
    except Exception as exc:  # noqa: BLE001
        print(f"\n  Fatal error: {exc}\n")
        logger.exception("Unhandled exception in agent loop")
        sys.exit(1)


if __name__ == "__main__":
    main()
