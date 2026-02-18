"""Shared brain file initialization.

Creates the ~/.amem/ directory, brain file, and metadata.
"""

from __future__ import annotations

import json
import subprocess
from datetime import datetime
from pathlib import Path


class BrainSetup:
    """Initialize the shared brain file and directory.

    Args:
        brain_path: Path to the .amem brain file.
        amem_binary: Path to the amem CLI binary.
    """

    def __init__(self, brain_path: Path, amem_binary: Path) -> None:
        self.brain_path = brain_path
        self.amem_binary = amem_binary

    def ensure_exists(self) -> None:
        """Create the brain file and ~/.amem/ directory if they don't exist.

        Also creates a session counter and installer metadata file.

        Raises:
            RuntimeError: If the amem CLI fails to create the brain file.
        """
        # Create directory
        self.brain_path.parent.mkdir(parents=True, exist_ok=True)

        # Create brain file if it doesn't exist
        if not self.brain_path.exists():
            result = subprocess.run(
                [str(self.amem_binary), "create", str(self.brain_path)],
                capture_output=True,
                text=True,
            )
            if result.returncode != 0:
                raise RuntimeError(f"Failed to create brain: {result.stderr}")

        # Create session counter
        counter_path = self.brain_path.parent / ".amem-session-counter"
        if not counter_path.exists():
            counter_path.write_text("0")

        # Create installer metadata
        meta_path = self.brain_path.parent / "installer-meta.json"
        if not meta_path.exists():
            meta = {
                "installed_at": datetime.now().isoformat(),
                "brain_path": str(self.brain_path),
                "amem_binary": str(self.amem_binary),
                "version": "0.1.0",
            }
            meta_path.write_text(json.dumps(meta, indent=2))
