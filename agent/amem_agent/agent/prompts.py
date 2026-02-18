"""
System prompt templates for the amem-agent.

Contains the default system prompt that instructs the model how to behave as
a persistent-memory assistant, and a builder that assembles the final prompt
from memory context and an optional user-supplied custom prompt.
"""

from __future__ import annotations

# ---------------------------------------------------------------------------
# Default system prompt
# ---------------------------------------------------------------------------

DEFAULT_SYSTEM_PROMPT: str = """\
You are a helpful AI assistant with persistent memory.

You have the ability to remember information across conversations through a
structured memory system.  When memories are available they will be provided
to you as context.  Use them naturally -- reference things the user has told
you before, recall their preferences, and build on prior conversations
without making a big deal out of it.

Guidelines for using your memory:

1. **Be natural.**  Do not announce that you are "checking memory" or
   "recalling" something.  Just use the information as if you genuinely
   remember it.
2. **Be accurate.**  Only reference memories you are confident about.  If a
   memory seems outdated or uncertain, mention it casually rather than
   stating it as fact.
3. **Respect corrections.**  If the user corrects something you remember,
   accept the correction gracefully and update your understanding.
4. **Stay relevant.**  Do not dump all your memories into a response.  Only
   bring up past information when it is directly useful or when the user asks
   about it.
5. **Be helpful first.**  Your primary job is to help the user with their
   current request.  Memory is a tool to do that better, not the focus of
   the conversation.
"""


# ---------------------------------------------------------------------------
# Prompt builder
# ---------------------------------------------------------------------------

def build_full_system_prompt(
    memory_context: str,
    custom_prompt: str | None = None,
) -> str:
    """Assemble the complete system prompt sent to the LLM.

    The final prompt is built by layering three optional sections:

    1. The base instruction set (either *custom_prompt* or
       :data:`DEFAULT_SYSTEM_PROMPT`).
    2. A ``## Relevant Memories`` section (only when *memory_context* is
       non-empty).

    Args:
        memory_context: Pre-formatted string of memories retrieved for the
            current turn.  Pass an empty string when no memories are
            available.
        custom_prompt: If provided, replaces :data:`DEFAULT_SYSTEM_PROMPT`
            entirely.  The caller is responsible for including any memory
            usage instructions they want the model to follow.

    Returns:
        The fully assembled system prompt string.
    """
    base = custom_prompt if custom_prompt else DEFAULT_SYSTEM_PROMPT

    sections: list[str] = [base.rstrip()]

    if memory_context and memory_context.strip():
        sections.append(
            "## Relevant Memories\n\n"
            "The following memories may be relevant to the current "
            "conversation.  Use them naturally.\n\n"
            f"{memory_context.strip()}"
        )

    return "\n\n".join(sections)
