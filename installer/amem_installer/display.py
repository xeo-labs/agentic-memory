"""Terminal output formatting for the installer.

Uses basic ANSI colors. No external dependencies (no rich library).
Falls back to plain text if terminal doesn't support colors.
"""

from __future__ import annotations

import sys
from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from amem_installer.scanner import DetectedTool, ToolStatus


class Display:
    """Terminal output formatting for the installer.

    Uses ANSI escape codes for color. Falls back to plain text
    when stdout is not a terminal.
    """

    GREEN = "\033[32m"
    RED = "\033[31m"
    YELLOW = "\033[33m"
    CYAN = "\033[36m"
    BOLD = "\033[1m"
    DIM = "\033[2m"
    RESET = "\033[0m"

    def __init__(self) -> None:
        self.color = sys.stdout.isatty()

    def _c(self, color: str, text: str) -> str:
        """Apply color if terminal supports it."""
        if self.color:
            return f"{color}{text}{self.RESET}"
        return text

    def header(self, text: str) -> None:
        """Print a bold header."""
        print(f"\n  {self._c(self.BOLD, text)}")
        print(f"  {'─' * len(text)}\n")

    def section(self, text: str) -> None:
        """Print a section heading."""
        print(f"\n  {self._c(self.CYAN, text)}")

    def success(self, text: str) -> None:
        """Print a success line."""
        print(f"  {self._c(self.GREEN, '✓')} {text}")

    def fail(self, tool: str, reason: str) -> None:
        """Print a failure line."""
        print(f"  {self._c(self.RED, '✗')} {tool} — {reason}")

    def skip(self, tool: str, reason: str) -> None:
        """Print a skipped tool line."""
        print(f"  {self._c(self.DIM, '⊘')} {tool} — {reason}")

    def already(self, tool: str) -> None:
        """Print an already-configured line."""
        print(f"  {self._c(self.DIM, '•')} {tool} — already configured")

    def warning(self, text: str) -> None:
        """Print a warning line."""
        print(f"  {self._c(self.YELLOW, '⚠')} {text}")

    def info(self, text: str) -> None:
        """Print an informational line."""
        print(f"  {text}")

    def detail(self, text: str) -> None:
        """Print a detail line (indented, dim)."""
        print(f"    {self._c(self.DIM, text)}")

    def tool_line(self, name: str, path: str | Path) -> None:
        """Print a tool name and its config path."""
        print(f"    {name:20s} {self._c(self.DIM, str(path))}")

    def scan_results(self, tools: list[DetectedTool]) -> None:
        """Print scan results for all tools."""
        from amem_installer.scanner import ToolStatus
        for t in tools:
            if t.status in (ToolStatus.FOUND, ToolStatus.RUNNING):
                status_str = f"{t.config_path or 'Running'}"
                if t.notes:
                    status_str += f" ({t.notes})"
                self.success(f"{t.name:20s} {self._c(self.DIM, status_str)}")
            elif t.status == ToolStatus.ALREADY_CONFIGURED:
                self.already(t.name)
            elif t.status == ToolStatus.NOT_RUNNING:
                self.warning(f"{t.name:20s} Installed but not running")
            else:
                self.skip(t.name, "Not found")

    def confirm(self, prompt: str) -> bool:
        """Ask for confirmation. Returns True if user agrees."""
        try:
            response = input(f"\n  {prompt} [Y/n] ").strip().lower()
            return response in ("", "y", "yes")
        except (EOFError, KeyboardInterrupt):
            return False

    def summary(
        self,
        configured: int,
        already: int,
        failed: int,
        brain_path: Path,
    ) -> None:
        """Print a summary of the install operation."""
        parts: list[str] = []
        if configured:
            parts.append(f"{configured} configured")
        if already:
            parts.append(f"{already} already set up")
        if failed:
            parts.append(f"{self._c(self.RED, f'{failed} failed')}")

        print(f"\n  {self._c(self.BOLD, 'Done!')} {', '.join(parts)}")
        print(f"  Brain: {self._c(self.CYAN, str(brain_path))}")
