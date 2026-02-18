"""Terminal display functions for amem-agent.

All user-facing output goes through this module.  It uses the ``rich``
library for colours, panels, markdown rendering, and tables.  Every
function is safe to call even when ``rich`` is unavailable -- in that
case output degrades gracefully to plain text.
"""

from __future__ import annotations

import logging
from typing import Any

from rich.console import Console
from rich.markdown import Markdown
from rich.panel import Panel
from rich.table import Table
from rich.text import Text
from rich.theme import Theme

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Console singleton
# ---------------------------------------------------------------------------

_THEME = Theme({
    "info": "dim cyan",
    "warning": "magenta",
    "error": "bold red",
    "success": "bold green",
    "memory": "dim yellow",
    "heading": "bold cyan",
    "muted": "dim",
})

console = Console(theme=_THEME)


# ---------------------------------------------------------------------------
# Welcome / Goodbye
# ---------------------------------------------------------------------------

def display_welcome(
    brain_info: Any | None,
    session_id: int,
    backend_name: str,
) -> None:
    """Show the welcome banner with brain statistics.

    Args:
        brain_info: A :class:`~amem_agent.brain.BrainInfo` instance, or
            ``None`` if the brain could not be read.
        session_id: The current session ID.
        backend_name: Human-readable name of the active LLM backend.
    """
    lines: list[str] = []

    if brain_info is not None:
        nodes = f"{brain_info.node_count:,}"
        sessions = f"{brain_info.session_count:,}"
        lines.append(f"Brain: {nodes} nodes | {sessions} sessions")
    else:
        lines.append("Brain: (new)")

    lines.append(f"Session: #{session_id} | Backend: {backend_name}")
    lines.append("Type /help for commands, /quit to exit")

    body = "\n".join(lines)
    panel = Panel(
        body,
        title="[bold]AgenticMemory Agent[/bold]",
        border_style="cyan",
        padding=(1, 2),
    )
    console.print()
    console.print(panel)
    console.print()


def display_goodbye(
    turn_count: int,
    events_extracted: int,
    edges_created: int,
    brain_info: Any | None = None,
) -> None:
    """Show the session summary on exit.

    Args:
        turn_count: Number of conversation turns completed.
        events_extracted: Total cognitive events written to the brain.
        edges_created: Total edges created during the session.
        brain_info: Optional :class:`~amem_agent.brain.BrainInfo` to show
            the current brain size.
    """
    lines: list[str] = [
        f"Turns: {turn_count}",
        f"Events extracted: {events_extracted}",
        f"Edges created: {edges_created}",
    ]
    if brain_info is not None:
        lines.append(f"Brain total: {brain_info.node_count:,} nodes")

    body = "\n".join(lines)
    panel = Panel(
        body,
        title="[bold]Session Complete[/bold]",
        border_style="green",
        padding=(0, 2),
    )
    console.print()
    console.print(panel)
    console.print()


# ---------------------------------------------------------------------------
# Response display
# ---------------------------------------------------------------------------

def display_response(content: str) -> None:
    """Render the assistant's response with markdown formatting.

    Args:
        content: The raw response text from the LLM.
    """
    console.print()
    md = Markdown(content)
    console.print(md, width=min(console.width, 100))
    console.print()


# ---------------------------------------------------------------------------
# Brain stats
# ---------------------------------------------------------------------------

def display_brain_stats(stats: dict) -> None:
    """Render brain statistics in a table.

    Args:
        stats: The dictionary returned by :meth:`Brain.stats`.
    """
    table = Table(
        title="Brain Statistics",
        border_style="cyan",
        show_header=True,
        header_style="bold cyan",
    )
    table.add_column("Metric", style="bold")
    table.add_column("Value", justify="right")

    # Top-level counts
    table.add_row("Nodes", f"{stats.get('nodes', 0):,}")
    table.add_row("Edges", f"{stats.get('edges', 0):,}")
    table.add_row("Sessions", f"{stats.get('sessions', 0):,}")

    file_size = stats.get("file_size", 0)
    if file_size >= 1_048_576:
        size_str = f"{file_size / 1_048_576:.1f} MB"
    elif file_size >= 1024:
        size_str = f"{file_size / 1024:.1f} KB"
    else:
        size_str = f"{file_size} B"
    table.add_row("File Size", size_str)

    # Node type breakdown (if present)
    node_types = stats.get("node_types", {})
    if node_types:
        table.add_section()
        for type_name, count in sorted(node_types.items()):
            table.add_row(f"  {type_name.capitalize()}", str(count))

    # Edge type breakdown (if present)
    edge_types = stats.get("edge_types", {})
    if edge_types:
        table.add_section()
        for type_name, count in sorted(edge_types.items()):
            table.add_row(f"  {type_name}", str(count))

    console.print()
    console.print(table)
    console.print()


# ---------------------------------------------------------------------------
# Help
# ---------------------------------------------------------------------------

def display_help() -> None:
    """Show the available slash commands."""
    table = Table(
        title="Commands",
        border_style="cyan",
        show_header=False,
        padding=(0, 2),
    )
    table.add_column("Command", style="bold cyan", min_width=12)
    table.add_column("Description")

    table.add_row("/stats", "Show brain statistics")
    table.add_row("/sessions", "List recent sessions")
    table.add_row("/memory", "Show recent memories")
    table.add_row("/help", "Show this help")
    table.add_row("/quit", "Exit (also: Ctrl+C, Ctrl+D)")

    console.print()
    console.print(table)
    console.print()


# ---------------------------------------------------------------------------
# Error display
# ---------------------------------------------------------------------------

def display_error(message: str) -> None:
    """Display an error message.

    Args:
        message: The error text to show.
    """
    console.print(f"[error]Error:[/error] {message}")


# ---------------------------------------------------------------------------
# Generic message
# ---------------------------------------------------------------------------

def display_message(message: str) -> None:
    """Display an informational message.

    Args:
        message: The text to show.
    """
    console.print(f"[info]{message}[/info]")


# ---------------------------------------------------------------------------
# Sessions list
# ---------------------------------------------------------------------------

def display_sessions(sessions: list[dict]) -> None:
    """Display a table of sessions.

    Args:
        sessions: A list of session dictionaries as returned by
            :meth:`Brain.get_sessions`.  Each dict is expected to have
            ``session_id`` and ``node_count``.
    """
    if not sessions:
        display_message("No sessions found.")
        return

    table = Table(
        title="Sessions",
        border_style="cyan",
        show_header=True,
        header_style="bold cyan",
    )
    table.add_column("Session", justify="right", style="bold")
    table.add_column("Nodes", justify="right")

    for session in sessions:
        sid = str(session.get("session_id", "?"))
        count = str(session.get("node_count", 0))
        table.add_row(sid, count)

    console.print()
    console.print(table)
    console.print()


# ---------------------------------------------------------------------------
# Recent memories
# ---------------------------------------------------------------------------

def display_recent_memories(memories: list[dict]) -> None:
    """Display the most recent memories (facts and decisions).

    Args:
        memories: A list of node dictionaries as returned by
            :meth:`Brain.search`.  Each dict is expected to have
            ``id``, ``type``, ``content``, and ``confidence``.
    """
    if not memories:
        display_message("No memories found.")
        return

    table = Table(
        title="Recent Memories",
        border_style="cyan",
        show_header=True,
        header_style="bold cyan",
    )
    table.add_column("ID", justify="right", style="bold", width=5)
    table.add_column("Type", width=12)
    table.add_column("Content")
    table.add_column("Confidence", justify="right", width=10)

    for mem in memories:
        node_id = str(mem.get("id", "?"))
        event_type = mem.get("type", "unknown")
        content = mem.get("content", "")
        confidence = mem.get("confidence")

        # Colour-code the event type
        type_colours = {
            "fact": "green",
            "decision": "yellow",
            "inference": "blue",
            "correction": "red",
            "skill": "magenta",
            "episode": "cyan",
        }
        colour = type_colours.get(event_type, "white")
        styled_type = f"[{colour}]{event_type}[/{colour}]"

        conf_str = f"{confidence:.0%}" if confidence is not None else "-"

        # Truncate very long content for display.
        if len(content) > 80:
            content = content[:77] + "..."

        table.add_row(node_id, styled_type, content, conf_str)

    console.print()
    console.print(table)
    console.print()


# ---------------------------------------------------------------------------
# Extraction display (debug / verbose mode)
# ---------------------------------------------------------------------------

def display_extraction(result: Any) -> None:
    """Show extracted cognitive events (used when ``show_extraction`` is on).

    Args:
        result: An :class:`~amem_agent.memory.extractor.ExtractionResult`
            instance, which has ``events``, ``corrections``, and
            ``session_summary`` attributes.
    """
    lines: list[str] = []

    if hasattr(result, "events") and result.events:
        lines.append("[bold]Extracted Events:[/bold]")
        for event in result.events:
            etype = getattr(event, "event_type", "?")
            content = getattr(event, "content", "?")
            conf = getattr(event, "confidence", 0.0)
            lines.append(f"  [{etype}] {content} ({conf:.0%})")

    if hasattr(result, "corrections") and result.corrections:
        lines.append("[bold]Corrections:[/bold]")
        for correction in result.corrections:
            old = getattr(correction, "old_description", "?")
            new = getattr(correction, "new_content", "?")
            lines.append(f"  {old} -> {new}")

    if hasattr(result, "session_summary") and result.session_summary:
        lines.append(f"[muted]Summary: {result.session_summary}[/muted]")

    if not lines:
        return

    body = "\n".join(lines)
    panel = Panel(
        body,
        title="[bold memory]Memory Formation[/bold memory]",
        border_style="yellow",
        padding=(0, 1),
    )
    console.print(panel)
