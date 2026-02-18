"""Platform detection tests."""

from pathlib import Path

from amem_installer.platform import PlatformInfo


def test_platform_detect():
    """Should detect real platform info."""
    info = PlatformInfo.detect()
    assert info.home == Path.home()
    assert info.amem_dir == Path.home() / ".amem"
    assert info.brain_path == Path.home() / ".amem" / "brain.amem"


def test_platform_os_detection():
    """Should detect a valid OS string."""
    info = PlatformInfo.detect()
    assert info.os in ("darwin", "linux", "win32")


def test_platform_config_dir_exists():
    """Config dir should be a valid path."""
    info = PlatformInfo.detect()
    assert isinstance(info.config_dir, Path)


def test_platform_sandbox(sandbox):
    """Sandbox platform should point to sandbox paths."""
    p = sandbox.platform
    assert p.home == sandbox.home
    assert p.amem_dir == sandbox.home / ".amem"
    assert p.brain_path == sandbox.home / ".amem" / "brain.amem"
