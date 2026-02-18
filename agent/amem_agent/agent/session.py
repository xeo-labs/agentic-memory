"""
Session management for the amem-agent.

Provides a persistent, file-backed session counter so that each agent run
receives a unique, monotonically increasing session ID.  The counter file
lives alongside the brain database so it naturally follows the brain across
moves or copies.
"""

from __future__ import annotations

import logging
from pathlib import Path

logger = logging.getLogger(__name__)


class SessionManager:
    """Manages persistent session IDs backed by a counter file on disk.

    The counter file is stored next to the brain database (in the parent
    directory of *brain_path*) and is named ``.amem-session-counter``.

    Typical usage::

        sm = SessionManager("/data/my-brain.db")
        sid = sm.next_session_id()   # 0 on first call, 1 on second, ...
        print(sm.current_session_id())  # same value until next_session_id()
    """

    def __init__(self, brain_path: str) -> None:
        """Initialise the session manager.

        Args:
            brain_path: Filesystem path to the brain database file.  The
                counter file will be placed in the same parent directory.
        """
        brain = Path(brain_path)
        self._counter_file: Path = brain.parent / ".amem-session-counter"
        logger.debug("Session counter file: %s", self._counter_file)

    # ------------------------------------------------------------------
    # Public API
    # ------------------------------------------------------------------

    def next_session_id(self) -> int:
        """Read the current counter, increment it, persist, and return the new value.

        If the counter file does not yet exist the sequence starts at ``0``.

        Returns:
            The newly assigned session ID.
        """
        current = self._read_counter()
        next_id = current + 1
        self._write_counter(next_id)
        logger.info("Session ID incremented: %d -> %d", current, next_id)
        return next_id

    def current_session_id(self) -> int:
        """Return the most recently assigned session ID **without** incrementing.

        If no session has been started yet (counter file missing) this
        returns ``0``.

        Returns:
            The current session ID.
        """
        return self._read_counter()

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

    def _read_counter(self) -> int:
        """Read the integer value from the counter file.

        Returns ``0`` when the file does not exist or contains invalid data.
        """
        try:
            if self._counter_file.exists():
                text = self._counter_file.read_text().strip()
                if text:
                    return int(text)
        except (ValueError, OSError) as exc:
            logger.warning(
                "Could not read session counter from %s: %s",
                self._counter_file,
                exc,
            )
        return 0

    def _write_counter(self, value: int) -> None:
        """Atomically persist *value* to the counter file.

        Creates parent directories if they do not already exist.

        Args:
            value: The counter value to write.
        """
        try:
            self._counter_file.parent.mkdir(parents=True, exist_ok=True)
            self._counter_file.write_text(str(value))
        except OSError as exc:
            logger.error(
                "Failed to write session counter to %s: %s",
                self._counter_file,
                exc,
            )
