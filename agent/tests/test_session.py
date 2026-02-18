"""Tests for session management (amem_agent.agent.session).

Uses a real temporary directory via pytest's ``tmp_path`` fixture.
"""

from __future__ import annotations

from pathlib import Path

import pytest

from amem_agent.agent.session import SessionManager


# ---------------------------------------------------------------------------
# Session counter
# ---------------------------------------------------------------------------


class TestSessionCounterNew:
    def test_session_counter_new(self, tmp_path):
        """A fresh SessionManager (no counter file) should start at 0."""
        brain_path = str(tmp_path / "brain.amem")
        sm = SessionManager(brain_path)

        assert sm.current_session_id() == 0


class TestSessionCounterIncrement:
    def test_session_counter_increment(self, tmp_path):
        """next_session_id should increment from 0 -> 1 -> 2."""
        brain_path = str(tmp_path / "brain.amem")
        sm = SessionManager(brain_path)

        first = sm.next_session_id()
        assert first == 1

        second = sm.next_session_id()
        assert second == 2

        third = sm.next_session_id()
        assert third == 3


class TestSessionCounterPersistence:
    def test_session_counter_persistence(self, tmp_path):
        """A new SessionManager instance should pick up where the last left off."""
        brain_path = str(tmp_path / "brain.amem")

        sm1 = SessionManager(brain_path)
        sm1.next_session_id()  # 1
        sm1.next_session_id()  # 2

        # Create a brand new manager pointed at the same brain directory.
        sm2 = SessionManager(brain_path)
        assert sm2.current_session_id() == 2
        assert sm2.next_session_id() == 3


class TestSessionCounterMissingFile:
    def test_session_counter_missing_file(self, tmp_path):
        """If the counter file is deleted mid-session, it should reset to 0."""
        brain_path = str(tmp_path / "brain.amem")
        sm = SessionManager(brain_path)
        sm.next_session_id()  # 1

        # Manually delete the counter file
        counter_file = tmp_path / ".amem-session-counter"
        assert counter_file.exists()
        counter_file.unlink()

        # current_session_id should now be 0 (file gone)
        assert sm.current_session_id() == 0
        # next call should start from 0 again -> 1
        assert sm.next_session_id() == 1
