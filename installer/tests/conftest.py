"""Shared test fixtures for the installer test suite.

All tests use a sandboxed home directory — a temp folder that acts
as ~. No real system configs are ever modified.
"""

import json
import shutil
import tempfile
from pathlib import Path

import pytest

from amem_installer.platform import PlatformInfo


class SandboxEnv:
    """A sandboxed environment for testing the installer.

    Creates a fake home directory with helpers for simulating
    tool installations.
    """

    def __init__(self, root: Path) -> None:
        self.root = root
        self.home = root / "home"
        self.home.mkdir()

    @property
    def platform(self) -> PlatformInfo:
        """Get a PlatformInfo pointing to the sandbox."""
        return PlatformInfo(
            os="darwin",
            home=self.home,
            config_dir=self.home / ".config",
            data_dir=self.home / ".local" / "share",
            amem_dir=self.home / ".amem",
            brain_path=self.home / ".amem" / "brain.amem",
        )

    def install_claude_code(self, existing_config: dict | None = None) -> Path:
        """Simulate Claude Code being installed."""
        config_dir = self.home / ".claude"
        config_dir.mkdir(parents=True, exist_ok=True)
        config_path = config_dir / "claude_desktop_config.json"
        config = existing_config or {"mcpServers": {}}
        config_path.write_text(json.dumps(config, indent=2))
        return config_path

    def install_cursor(self, existing_config: dict | None = None) -> Path:
        """Simulate Cursor being installed."""
        config_dir = self.home / ".cursor"
        config_dir.mkdir(parents=True, exist_ok=True)
        config_path = config_dir / "mcp.json"
        config = existing_config or {"mcpServers": {}}
        config_path.write_text(json.dumps(config, indent=2))
        return config_path

    def install_continue(self, existing_config: dict | None = None) -> Path:
        """Simulate Continue extension being installed."""
        config_dir = self.home / ".continue"
        config_dir.mkdir(parents=True, exist_ok=True)
        config_path = config_dir / "config.json"
        config = existing_config or {"models": [], "contextProviders": []}
        config_path.write_text(json.dumps(config, indent=2))
        return config_path

    def install_openclaw(self, existing_config: dict | None = None) -> Path:
        """Simulate OpenClaw being installed."""
        import yaml
        config_dir = self.home / ".config" / "openclaw"
        config_dir.mkdir(parents=True, exist_ok=True)
        config_path = config_dir / "config.yaml"
        config = existing_config or {"agent": {"name": "default"}}
        config_path.write_text(yaml.dump(config))
        return config_path

    def create_project_with_requirements(self, packages: list[str]) -> Path:
        """Create a fake project directory with requirements.txt."""
        proj_dir = self.root / "project"
        proj_dir.mkdir(parents=True, exist_ok=True)
        req_path = proj_dir / "requirements.txt"
        req_path.write_text("\n".join(packages))
        return proj_dir

    def create_amem_binary(self) -> Path:
        """Create a fake amem binary for testing."""
        bin_dir = self.home / ".cargo" / "bin"
        bin_dir.mkdir(parents=True, exist_ok=True)
        binary = bin_dir / "amem"
        binary.write_text("#!/bin/bash\necho 'amem 0.1.0'")
        binary.chmod(0o755)
        return binary


@pytest.fixture
def sandbox():
    """Create a sandboxed home directory for testing."""
    tmpdir = tempfile.mkdtemp(prefix="amem_install_test_")
    yield SandboxEnv(Path(tmpdir))
    shutil.rmtree(tmpdir, ignore_errors=True)
