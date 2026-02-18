"""
Agent package -- conversation loop and session management.

Exports:
    AgentLoop: The main interactive conversation loop.
    SessionManager: Persistent session counter tied to a brain directory.
"""

from amem_agent.agent.loop import AgentLoop
from amem_agent.agent.session import SessionManager

__all__ = ["AgentLoop", "SessionManager"]
