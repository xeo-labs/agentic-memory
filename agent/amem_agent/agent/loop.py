"""
Main agent conversation loop.

Implements the interactive REPL that ties together the LLM backend, the
AgenticMemory brain, memory extraction, and terminal display into a single
cohesive conversational experience.
"""

from __future__ import annotations

import logging
from typing import TYPE_CHECKING

from amem_agent.agent.prompts import build_full_system_prompt
from amem_agent.llm.base import LLMError, Message
from amem_agent.memory.context import build_memory_context, extract_and_store
from amem_agent.memory.extractor import (
    ExtractionResult,
    find_best_match,
    format_existing_memories,
)
from amem_agent.utils.display import (
    display_brain_stats,
    display_error,
    display_extraction,
    display_goodbye,
    display_help,
    display_recent_memories,
    display_response,
    display_sessions,
    display_welcome,
)

if TYPE_CHECKING:
    from amem_agent.brain import Brain
    from amem_agent.llm.base import LLMBackend

logger = logging.getLogger(__name__)


class AgentLoop:
    """Interactive conversation loop with persistent memory.

    Coordinates user input, LLM calls, memory retrieval, event extraction,
    and terminal output into a single run-loop.

    Args:
        brain: The AgenticMemory brain instance for reading/writing memories.
        llm: An initialised LLM backend for chat completions.
        config: The fully resolved application configuration object.
        session_id: Numeric session identifier for this conversation.
    """

    def __init__(
        self,
        brain: Brain,
        llm: LLMBackend,
        config: object,
        session_id: int,
    ) -> None:
        self.brain: Brain = brain
        self.llm: LLMBackend = llm
        self.config = config
        self.session_id: int = session_id

        self.history: list[Message] = []
        self.turn_count: int = 0
        self.events_extracted: int = 0
        self.edges_created: int = 0

    # ------------------------------------------------------------------
    # Main entry point
    # ------------------------------------------------------------------

    def run(self) -> None:
        """Start the interactive conversation loop.

        Blocks until the user exits via a quit command, ``Ctrl-C``, or
        ``Ctrl-D``.
        """
        self._display_welcome()

        try:
            while True:
                user_input = self.get_input()
                if user_input is None:
                    # Ctrl-D / EOF
                    break

                if not user_input:
                    continue

                # Slash commands (/quit, /stats, etc.)
                if self.handle_command(user_input):
                    continue

                self.turn_count += 1

                # --- Build memory context ---
                memory_context = ""
                if self.config.memory.enabled:
                    try:
                        memory_context = build_memory_context(
                            brain=self.brain,
                            session_id=self.session_id,
                            user_message=user_input,
                            llm=self.llm,
                        )
                    except Exception as exc:  # noqa: BLE001
                        logger.warning("Memory context retrieval failed: %s", exc)

                # --- Assemble messages and call LLM ---
                system_prompt = build_full_system_prompt(
                    memory_context=memory_context,
                    custom_prompt=self.config.agent.system_prompt,
                )
                messages = self.build_messages(system_prompt, user_input)

                try:
                    llm_response = self.llm.chat(messages)
                except LLMError as exc:
                    display_error(f"LLM call failed: {exc}")
                    logger.error("LLM error on turn %d: %s", self.turn_count, exc)
                    continue

                assistant_text = llm_response.content

                # --- Display and record ---
                display_response(assistant_text)

                self.update_history(user_input, assistant_text)

                # --- Form memories ---
                if self.config.memory.enabled and self.config.memory.extract_events:
                    self.form_memories(user_input, assistant_text)

        except KeyboardInterrupt:
            # Ctrl-C
            pass

        self.shutdown()

    # ------------------------------------------------------------------
    # Input
    # ------------------------------------------------------------------

    def get_input(self) -> str | None:
        """Prompt the user and return their input, or ``None`` on EOF.

        Returns:
            Stripped user input string, or ``None`` if the user sent EOF
            (Ctrl-D).
        """
        try:
            return input("\n  You: ").strip()
        except EOFError:
            return None

    # ------------------------------------------------------------------
    # Slash commands
    # ------------------------------------------------------------------

    def handle_command(self, user_input: str) -> bool:
        """Process slash commands.

        Args:
            user_input: The raw input string from the user.

        Returns:
            ``True`` if the input was a recognised command (and therefore
            should not be sent to the LLM), ``False`` otherwise.
        """
        cmd = user_input.lower().strip()

        if cmd in ("/quit", "/exit", "/q"):
            raise KeyboardInterrupt  # triggers clean shutdown

        if cmd == "/stats":
            self._show_stats()
            return True

        if cmd == "/sessions":
            self._show_sessions()
            return True

        if cmd.startswith("/memory"):
            self._show_memory()
            return True

        if cmd == "/help":
            display_help()
            return True

        if cmd.startswith("/forget"):
            query = cmd[len("/forget"):].strip()
            if query:
                self._handle_forget(query)
            else:
                print("\n  Usage: /forget <query>")
            return True

        return False

    # ------------------------------------------------------------------
    # Message construction
    # ------------------------------------------------------------------

    def build_messages(
        self,
        system_prompt: str,
        user_input: str,
    ) -> list[Message]:
        """Assemble the message list to send to the LLM.

        Includes the system prompt, the most recent history entries (bounded
        by ``config.agent.max_history``), and the current user message.

        Args:
            system_prompt: The fully assembled system prompt.
            user_input: The current turn's user input.

        Returns:
            An ordered list of :class:`Message` objects.
        """
        max_history = getattr(self.config.agent, "max_history", 20)
        # Each turn = 2 messages (user + assistant), so keep last N*2 entries.
        recent_history = self.history[-(max_history * 2):]

        messages: list[Message] = [Message(role="system", content=system_prompt)]
        messages.extend(recent_history)
        messages.append(Message(role="user", content=user_input))

        return messages

    # ------------------------------------------------------------------
    # History management
    # ------------------------------------------------------------------

    def update_history(self, user_input: str, assistant_response: str) -> None:
        """Append the latest exchange to history and trim to the configured max.

        Args:
            user_input: What the user said.
            assistant_response: What the assistant replied.
        """
        self.history.append(Message(role="user", content=user_input))
        self.history.append(Message(role="assistant", content=assistant_response))

        max_history = getattr(self.config.agent, "max_history", 20)
        max_entries = max_history * 2
        if len(self.history) > max_entries:
            self.history = self.history[-max_entries:]

    # ------------------------------------------------------------------
    # Memory formation
    # ------------------------------------------------------------------

    def form_memories(self, user_input: str, assistant_response: str) -> None:
        """Extract events from the latest exchange and persist them to the brain.

        Delegates to :func:`extract_and_store` which handles the complete
        pipeline: extraction, storage, relationship linking, and corrections.

        This method is deliberately fault-tolerant: any exception during
        extraction or writing is logged but never propagated, so the
        conversation loop is never interrupted by memory failures.

        Args:
            user_input: What the user said.
            assistant_response: What the assistant replied.
        """
        try:
            extract_and_store(
                brain=self.brain,
                llm=self.llm,
                user_message=user_input,
                assistant_response=assistant_response,
                session_id=self.session_id,
            )
        except Exception as exc:  # noqa: BLE001
            logger.warning("Memory formation failed: %s", exc)

    # ------------------------------------------------------------------
    # Command helpers
    # ------------------------------------------------------------------

    def _show_stats(self) -> None:
        """Display brain statistics via the /stats command."""
        try:
            stats = self.brain.stats()
            display_brain_stats(stats)
        except Exception as exc:  # noqa: BLE001
            display_error(f"Could not retrieve brain stats: {exc}")

    def _show_sessions(self) -> None:
        """Display past sessions via the /sessions command."""
        try:
            sessions = self.brain.get_sessions()
            display_sessions(sessions)
        except Exception as exc:  # noqa: BLE001
            display_error(f"Could not list sessions: {exc}")

    def _show_memory(self) -> None:
        """Display recent memories via the /memory command."""
        try:
            memories = self.brain.search(sort="recent", limit=10)
            display_recent_memories(memories)
        except Exception as exc:  # noqa: BLE001
            display_error(f"Could not retrieve memories: {exc}")

    def _handle_forget(self, query: str) -> None:
        """Attempt to find and report a memory matching *query*.

        Note: The current brain CLI does not support node deletion, so this
        command only identifies the matching memory.  A future version may
        add soft-delete support.

        Args:
            query: Free-text description of the memory to forget.
        """
        try:
            candidates = self.brain.search(sort="recent", limit=50)
            match = find_best_match(query, candidates)
            if match is None:
                print(f"\n  No memory found matching: {query}")
                return
            node_id = match.get("id", "?")
            content = match.get("content", "")
            print(f"\n  Found memory #{node_id}: {content[:60]}")
            print("  (Note: deletion is not yet supported by the brain backend.)")
        except Exception as exc:  # noqa: BLE001
            display_error(f"Could not search for memory: {exc}")

    # ------------------------------------------------------------------
    # Welcome / Goodbye display delegates
    # ------------------------------------------------------------------

    def _display_welcome(self) -> None:
        """Show the welcome banner with brain info."""
        brain_info = None
        try:
            brain_info = self.brain.info()
        except Exception as exc:  # noqa: BLE001
            logger.debug("Could not read brain info for welcome: %s", exc)

        display_welcome(
            brain_info=brain_info,
            session_id=self.session_id,
            backend_name=self.llm.name(),
        )

    # ------------------------------------------------------------------
    # Shutdown
    # ------------------------------------------------------------------

    def shutdown(self) -> None:
        """Clean up before exiting.

        If session compression is enabled, compress the current session's
        memories into a summary episode.  Then display the goodbye message.
        """
        if (
            self.config.memory.enabled
            and self.config.memory.compress_on_exit
            and self.turn_count > 0
        ):
            try:
                self._compress_session()
            except Exception as exc:  # noqa: BLE001
                logger.warning("Session compression failed: %s", exc)

        brain_info = None
        try:
            brain_info = self.brain.info()
        except Exception:  # noqa: BLE001
            pass

        display_goodbye(
            turn_count=self.turn_count,
            events_extracted=self.events_extracted,
            edges_created=self.edges_created,
            brain_info=brain_info,
        )

    def _compress_session(self) -> None:
        """Compress the session's conversation into a summary episode.

        Sends the full conversation history to the LLM and asks it to
        produce a concise summary, which is then stored as an episode node.
        """
        if not self.history:
            return

        summary_messages = [
            Message(
                role="system",
                content=(
                    "Summarize the following conversation in 2-3 sentences. "
                    "Focus on what was discussed, any decisions made, and "
                    "important information shared."
                ),
            ),
            Message(
                role="user",
                content="\n".join(
                    f"{m.role}: {m.content}" for m in self.history
                ),
            ),
        ]

        try:
            response = self.llm.chat(summary_messages)
            summary = response.content.strip()
            episode_text = f"[Session {self.session_id}] {summary}"

            self.brain.add_episode(
                content=episode_text,
                session_id=self.session_id,
            )
            logger.info("Session %d compressed into episode.", self.session_id)
        except LLMError as exc:
            logger.warning("LLM call for session compression failed: %s", exc)
