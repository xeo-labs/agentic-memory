"""Custom exceptions for the AgenticMemory SDK.

Hierarchy::

    AmemError
    ├── BrainNotFoundError   — .amem file doesn't exist
    ├── NodeNotFoundError    — node ID not in the graph
    ├── AmemNotFoundError    — amem CLI binary not found
    ├── CLIError             — amem CLI returned non-zero exit
    ├── ValidationError      — invalid input (e.g. self-edges)
    └── ProviderError        — LLM provider failure
"""

from __future__ import annotations


class AmemError(Exception):
    """Base exception for all AgenticMemory errors."""


class BrainNotFoundError(AmemError):
    """The .amem brain file does not exist.

    Raised when an operation requires the file to be present and
    ``auto_create`` is disabled.
    """

    def __init__(self, path: str) -> None:
        self.path = path
        super().__init__(f"Brain file not found: {path}")


class NodeNotFoundError(AmemError):
    """A node with the given ID does not exist in the brain.

    Raised by ``Brain.get()`` and similar read operations.
    """

    def __init__(self, node_id: int) -> None:
        self.node_id = node_id
        super().__init__(f"Node not found: {node_id}")


class AmemNotFoundError(AmemError):
    """The ``amem`` CLI binary could not be found.

    This typically means AgenticMemory core is not installed. The
    binary is required for all brain operations.
    """

    def __init__(self, searched: list[str] | None = None) -> None:
        self.searched = searched or []
        locations = ", ".join(self.searched) if self.searched else "PATH"
        super().__init__(
            f"amem CLI binary not found. Searched: {locations}. "
            "Install AgenticMemory core: cargo install amem"
        )


class CLIError(AmemError):
    """The ``amem`` CLI command failed with a non-zero exit code.

    Attributes:
        command: The full command that was executed.
        stderr: Standard error output from the command.
        returncode: Process exit code.
    """

    def __init__(self, command: str, stderr: str, returncode: int) -> None:
        self.command = command
        self.stderr = stderr
        self.returncode = returncode
        super().__init__(
            f"amem command failed (exit {returncode}): {command}\n{stderr}"
        )


class ValidationError(AmemError):
    """Invalid input to a brain operation.

    Examples: self-edges, empty content, negative confidence.
    """


class ProviderError(AmemError):
    """An LLM provider operation failed.

    Covers API errors, missing API keys, authentication failures,
    and JSON parse failures from LLM responses.
    """
