"""Platform-specific path detection.

Handles macOS, Linux, and Windows path differences.
"""

from __future__ import annotations

import os
import sys
from dataclasses import dataclass
from pathlib import Path


@dataclass
class PlatformInfo:
    """Platform-specific paths and capabilities.

    Attributes:
        os: Platform identifier — "darwin", "linux", "win32".
        home: User home directory.
        config_dir: Platform config dir (XDG on Linux, Library on macOS).
        data_dir: Platform data dir.
        amem_dir: AgenticMemory directory (~/.amem/).
        brain_path: Default brain file path.
    """
    os: str
    home: Path
    config_dir: Path
    data_dir: Path
    amem_dir: Path
    brain_path: Path

    @classmethod
    def detect(cls) -> PlatformInfo:
        """Auto-detect platform paths.

        Returns:
            PlatformInfo with correct paths for the current OS.
        """
        home = Path.home()
        os_name = sys.platform  # "darwin", "linux", "win32"

        if os_name == "darwin":
            config_dir = home / "Library" / "Application Support"
            data_dir = config_dir
        elif os_name == "linux":
            config_dir = Path(os.environ.get("XDG_CONFIG_HOME", str(home / ".config")))
            data_dir = Path(os.environ.get("XDG_DATA_HOME", str(home / ".local" / "share")))
        else:  # Windows
            config_dir = Path(os.environ.get("APPDATA", str(home / "AppData" / "Roaming")))
            data_dir = config_dir

        amem_dir = home / ".amem"
        brain_path = amem_dir / "brain.amem"

        return cls(
            os=os_name,
            home=home,
            config_dir=config_dir,
            data_dir=data_dir,
            amem_dir=amem_dir,
            brain_path=brain_path,
        )
