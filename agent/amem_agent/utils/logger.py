"""Logging configuration for amem-agent.

Provides a single :func:`setup_logging` function that configures the
standard-library :mod:`logging` module for the entire application.
"""

from __future__ import annotations

import logging
import sys


def setup_logging(verbose: bool = False) -> None:
    """Configure the root logger for amem-agent.

    In normal mode only warnings and above are shown.  In verbose mode
    debug messages are included, along with timestamps and source locations.

    Args:
        verbose: If ``True``, set the log level to ``DEBUG`` and use a
            detailed format string.  Otherwise use ``WARNING`` with a
            minimal format.
    """
    if verbose:
        level = logging.DEBUG
        fmt = (
            "%(asctime)s %(levelname)-8s %(name)s:%(lineno)d  %(message)s"
        )
        datefmt = "%H:%M:%S"
    else:
        level = logging.WARNING
        fmt = "%(levelname)s: %(message)s"
        datefmt = None

    handler = logging.StreamHandler(sys.stderr)
    handler.setFormatter(logging.Formatter(fmt=fmt, datefmt=datefmt))

    root = logging.getLogger()
    # Remove any handlers that were added before us (e.g. by library imports).
    root.handlers.clear()
    root.addHandler(handler)
    root.setLevel(level)

    # Silence noisy third-party loggers even in verbose mode.
    for noisy in ("httpx", "httpcore", "urllib3", "openai", "anthropic"):
        logging.getLogger(noisy).setLevel(logging.WARNING)

    logging.getLogger(__name__).debug(
        "Logging initialised (level=%s)", logging.getLevelName(level)
    )
