"""Backup system tests."""

from pathlib import Path

import pytest

from amem_installer.backup import backup_config, restore_config


def test_backup_creates_file(sandbox):
    """backup_config should create a backup file."""
    original = sandbox.home / "test_config.json"
    original.write_text('{"key": "value"}')
    backup_path = backup_config(original)
    assert backup_path.exists()
    assert backup_path.read_text() == '{"key": "value"}'


def test_backup_preserves_original(sandbox):
    """backup_config should not modify the original."""
    original = sandbox.home / "test_config.json"
    original.write_text('{"key": "value"}')
    backup_config(original)
    assert original.read_text() == '{"key": "value"}'


def test_backup_multiple_versions(sandbox):
    """Multiple backups should have different filenames."""
    original = sandbox.home / "test_config.json"
    original.write_text("v1")
    b1 = backup_config(original)
    original.write_text("v2")
    b2 = backup_config(original)
    assert b1 != b2
    assert b1.read_text() == "v1"
    assert b2.read_text() == "v2"


def test_restore_config(sandbox):
    """restore_config should restore from backup."""
    original = sandbox.home / "test_config.json"
    original.write_text("original content")
    backup_path = backup_config(original)
    original.write_text("modified content")
    assert restore_config(original, backup_path)
    assert original.read_text() == "original content"


def test_restore_nonexistent_backup(sandbox):
    """restore_config should return False for missing backup."""
    original = sandbox.home / "test_config.json"
    original.write_text("content")
    assert not restore_config(original, Path("/nonexistent/backup"))
