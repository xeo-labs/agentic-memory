"""Low-level amem CLI wrapper. NOT part of the public API.

This module handles subprocess management, CLI binary discovery,
output parsing, and error translation. The Brain class calls this;
users never import it directly.

In a future version, this will be replaced by direct FFI bindings
to the Rust library. The public API (Brain class) will not change.
"""

from __future__ import annotations

import json
import logging
import os
import re
import shutil
import subprocess
from pathlib import Path

from agentic_memory.errors import AmemNotFoundError, CLIError

logger = logging.getLogger(__name__)

# Regex for parsing the node ID from ``amem add`` text output.
# Example output: "Added node 42 (fact) to brain.amem"
_NODE_ID_RE = re.compile(r"Added node (\d+)")

# Default subprocess timeout in seconds.
DEFAULT_TIMEOUT = 30


def find_amem_binary(override: str | Path | None = None) -> Path:
    """Find the amem CLI binary.

    Search order:
    1. Explicit override path (if provided)
    2. AMEM_BINARY environment variable
    3. System PATH (shutil.which)
    4. ~/.cargo/bin/amem (Rust cargo install location)
    5. /usr/local/bin/amem

    Args:
        override: Explicit path to the binary. Checked first if provided.

    Returns:
        Path to the amem binary.

    Raises:
        AmemNotFoundError: If the binary cannot be found anywhere.
    """
    searched: list[str] = []

    # 1. Explicit override
    if override is not None:
        p = Path(override)
        searched.append(str(p))
        if p.is_file() and os.access(str(p), os.X_OK):
            return p
        raise AmemNotFoundError(searched)

    # 2. AMEM_BINARY environment variable
    env_binary = os.environ.get("AMEM_BINARY")
    if env_binary:
        p = Path(env_binary)
        searched.append(str(p))
        if p.is_file() and os.access(str(p), os.X_OK):
            return p

    # 3. System PATH
    which_result = shutil.which("amem")
    searched.append("PATH")
    if which_result:
        return Path(which_result)

    # 4. ~/.cargo/bin/amem
    cargo_path = Path.home() / ".cargo" / "bin" / "amem"
    searched.append(str(cargo_path))
    if cargo_path.is_file() and os.access(str(cargo_path), os.X_OK):
        return cargo_path

    # 5. /usr/local/bin/amem
    local_path = Path("/usr/local/bin/amem")
    searched.append(str(local_path))
    if local_path.is_file() and os.access(str(local_path), os.X_OK):
        return local_path

    raise AmemNotFoundError(searched)


def run_command(
    binary: Path,
    args: list[str],
    timeout: int = DEFAULT_TIMEOUT,
) -> str:
    """Run an amem CLI command and return stdout.

    Args:
        binary: Path to the amem binary.
        args: Command arguments (e.g., ["info", "brain.amem", "--format", "json"]).
        timeout: Maximum seconds to wait.

    Returns:
        Stdout as string.

    Raises:
        CLIError: If the command fails (non-zero exit code).
        TimeoutError: If the command exceeds timeout.
    """
    full_cmd = [str(binary)] + args
    cmd_str = " ".join(full_cmd)
    logger.debug("Running: %s", cmd_str)

    try:
        result = subprocess.run(
            full_cmd,
            capture_output=True,
            text=True,
            timeout=timeout,
        )
    except subprocess.TimeoutExpired:
        raise CLIError(
            command=cmd_str,
            stderr=f"Command timed out after {timeout}s",
            returncode=-1,
        )
    except FileNotFoundError:
        raise CLIError(
            command=cmd_str,
            stderr=f"amem binary not found at '{binary}'",
            returncode=-1,
        )

    if result.returncode != 0:
        raise CLIError(
            command=cmd_str,
            stderr=result.stderr.strip(),
            returncode=result.returncode,
        )

    return result.stdout.strip()


def run_command_json(
    binary: Path,
    args: list[str],
    timeout: int = DEFAULT_TIMEOUT,
) -> dict | list:  # type: ignore[type-arg]
    """Run an amem CLI command with --format json and parse the output.

    The ``--format json`` flag is inserted before the subcommand
    (first element of args).

    Args:
        binary: Path to the amem binary.
        args: Command arguments. The first element should be the
            subcommand (e.g., "info").
        timeout: Maximum seconds to wait.

    Returns:
        The parsed JSON output (dict or list).

    Raises:
        CLIError: If the command fails or output is not valid JSON.
    """
    json_args = ["--format", "json"] + args
    raw = run_command(binary, json_args, timeout)

    try:
        return json.loads(raw)  # type: ignore[no-any-return]
    except json.JSONDecodeError as exc:
        raise CLIError(
            command=" ".join([str(binary)] + json_args),
            stderr=f"Failed to parse JSON output: {exc}\nRaw output: {raw!r}",
            returncode=-1,
        )


def parse_node_id(output: str) -> int:
    """Parse a node ID from amem add command output.

    Handles both text format ("Added node 42 ...") and JSON format.

    Args:
        output: Raw CLI output.

    Returns:
        The node ID.

    Raises:
        CLIError: If the output can't be parsed.
    """
    # Try text format first
    match = _NODE_ID_RE.search(output)
    if match:
        return int(match.group(1))

    # Try JSON format
    try:
        data = json.loads(output)
        if isinstance(data, dict):
            node_id = data.get("id", data.get("node_id"))
            if node_id is not None:
                return int(node_id)
    except (json.JSONDecodeError, ValueError):
        pass

    raise CLIError(
        command="add",
        stderr=f"Could not parse node ID from output: {output!r}",
        returncode=-1,
    )
