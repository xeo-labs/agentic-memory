"""Config backup and restore logic.

Before any config modification, the original file is backed up.
Backups are registered in a central registry so uninstall can
restore them.
"""

from __future__ import annotations

import json
import shutil
from datetime import datetime
from pathlib import Path

BACKUP_SUFFIX = ".amem-backup"
BACKUP_REGISTRY = ".amem/backup-registry.json"


def backup_config(config_path: Path) -> Path:
    """Create a backup of a config file.

    Backup is stored next to the original with .amem-backup suffix.
    If a backup already exists, adds a timestamp.

    Args:
        config_path: Path to the config file to back up.

    Returns:
        Path to the backup file.
    """
    backup_path = config_path.with_suffix(config_path.suffix + BACKUP_SUFFIX)

    if backup_path.exists():
        ts = datetime.now().strftime("%Y%m%d_%H%M%S")
        backup_path = config_path.with_suffix(f"{config_path.suffix}.amem-backup-{ts}")

    shutil.copy2(str(config_path), str(backup_path))

    _register_backup(config_path, backup_path)

    return backup_path


def restore_config(config_path: Path, backup_path: Path) -> bool:
    """Restore a config file from backup.

    Args:
        config_path: Path to restore to.
        backup_path: Path to the backup file.

    Returns:
        True on success, False if backup doesn't exist.
    """
    if not backup_path.exists():
        return False
    shutil.copy2(str(backup_path), str(config_path))
    return True


def list_backups(home: Path | None = None) -> list[dict]:  # type: ignore[type-arg]
    """List all registered backups.

    Args:
        home: Home directory override (for testing).

    Returns:
        List of backup records.
    """
    if home is None:
        home = Path.home()
    registry_path = home / BACKUP_REGISTRY
    if not registry_path.exists():
        return []
    try:
        return json.loads(registry_path.read_text())  # type: ignore[no-any-return]
    except (json.JSONDecodeError, OSError):
        return []


def _register_backup(original: Path, backup: Path) -> None:
    """Register a backup in the central registry."""
    registry_path = original.parent
    # Walk up to find .amem dir, or use home
    home = Path.home()
    registry_path = home / BACKUP_REGISTRY
    registry_path.parent.mkdir(parents=True, exist_ok=True)

    registry: list[dict] = []  # type: ignore[type-arg]
    if registry_path.exists():
        try:
            registry = json.loads(registry_path.read_text())
        except (json.JSONDecodeError, OSError):
            registry = []

    registry.append({
        "original": str(original),
        "backup": str(backup),
        "timestamp": datetime.now().isoformat(),
        "tool": _infer_tool_from_path(original),
    })

    registry_path.write_text(json.dumps(registry, indent=2))


def _infer_tool_from_path(path: Path) -> str:
    """Guess tool name from config file path."""
    path_str = str(path).lower()
    if ".claude" in path_str:
        return "claude_code"
    elif ".cursor" in path_str:
        return "cursor"
    elif ".windsurf" in path_str or "codeium" in path_str:
        return "windsurf"
    elif ".continue" in path_str:
        return "continue"
    elif "openclaw" in path_str:
        return "openclaw"
    elif "ollama" in path_str:
        return "ollama"
    return "unknown"
