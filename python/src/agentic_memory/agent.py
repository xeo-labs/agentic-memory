"""MemoryAgent — high-level agent with full memory loop.

Wraps Brain + LLMProvider into a single ``chat()`` method that
automatically recalls relevant memories, calls the LLM, extracts
cognitive events, and stores them.
"""

from __future__ import annotations

import logging
from typing import Optional

from agentic_memory.brain import Brain
from agentic_memory.integrations.base import ChatMessage, ChatResponse, LLMProvider
from agentic_memory.integrations.context import build_memory_context
from agentic_memory.integrations.extractor import ExtractionResult, extract_events

logger = logging.getLogger(__name__)

_DEFAULT_SYSTEM_PROMPT = (
    "You are a helpful AI assistant with persistent memory. "
    "You can remember facts, decisions, and context from previous "
    "conversations with this user."
)


class MemoryAgent:
    """High-level agent that handles the full memory loop.

    Wraps Brain + LLMProvider into a single chat() method that
    automatically recalls relevant memories, calls the LLM, extracts
    cognitive events, and stores them.

    Args:
        brain: The Brain instance for memory storage.
        provider: The LLM provider for chat and extraction.
        system_prompt: Custom system prompt (optional).
        max_context_tokens: Maximum tokens for memory context (default: 2000).
        extract_events: Whether to extract events after each turn (default: True).

    Example:
        >>> from agentic_memory import Brain, MemoryAgent
        >>> from agentic_memory.integrations import AnthropicProvider
        >>>
        >>> brain = Brain("agent.amem")
        >>> provider = AnthropicProvider()
        >>> agent = MemoryAgent(brain=brain, provider=provider)
        >>>
        >>> response = agent.chat("My name is Marcus", session=1)
        >>> # Later, in a new session:
        >>> response = agent.chat("What's my name?", session=2)
        >>> # Returns a response that includes "Marcus"
    """

    def __init__(
        self,
        brain: Brain,
        provider: LLMProvider,
        system_prompt: str | None = None,
        max_context_tokens: int = 2000,
        extract_events: bool = True,
    ) -> None:
        self._brain = brain
        self._provider = provider
        self._system_prompt = system_prompt or _DEFAULT_SYSTEM_PROMPT
        self._max_context_tokens = max_context_tokens
        self._extract_events = extract_events
        self._last_extraction: ExtractionResult | None = None

    def chat(
        self,
        message: str,
        session: int,
        history: list[ChatMessage] | None = None,
    ) -> ChatResponse:
        """Send a message with full memory integration.

        1. Queries brain for relevant memories
        2. Builds system prompt with memory context
        3. Calls LLM with message + history + memory
        4. Extracts cognitive events from the exchange
        5. Stores events in the brain

        Args:
            message: The user's message.
            session: Current session ID.
            history: Optional within-session conversation history.

        Returns:
            The LLM's response.

        Note:
            Memory extraction runs after the response is generated.
            If extraction fails, the response is still returned.
        """
        # Step 1: Build memory context
        memory_context = build_memory_context(
            self._brain,
            session=session,
            user_message=message,
            max_tokens=self._max_context_tokens,
        )

        # Step 2: Build system prompt
        system_parts = [self._system_prompt]
        if memory_context:
            system_parts.append(
                "# Your Memories\n"
                "The following is what you remember about this user "
                "from previous interactions:\n\n" + memory_context
            )
        system_text = "\n\n".join(system_parts)

        # Step 3: Assemble messages
        messages: list[ChatMessage] = [
            ChatMessage(role="system", content=system_text),
        ]
        if history:
            messages.extend(history)
        messages.append(ChatMessage(role="user", content=message))

        # Step 4: Call LLM
        response = self._provider.chat(messages)

        # Step 5: Extract and store events (non-blocking — failures don't crash)
        if self._extract_events:
            self._extract_and_store(message, response.content, memory_context, session)

        return response

    @property
    def last_extraction(self) -> ExtractionResult | None:
        """The result of the most recent event extraction.

        Returns None if no extraction has been performed or if the
        last extraction failed.
        """
        return self._last_extraction

    def _extract_and_store(
        self,
        user_message: str,
        assistant_response: str,
        existing_memories: str,
        session: int,
    ) -> None:
        """Extract cognitive events and store them in the brain.

        This is called after every chat turn. Failures are logged but
        never propagated — the user always gets their response.
        """
        try:
            result = extract_events(
                self._provider,
                user_message=user_message,
                assistant_response=assistant_response,
                existing_memories=existing_memories,
            )
            self._last_extraction = result

            # Store extracted events
            for event in result.events:
                try:
                    if event.type == "fact":
                        self._brain.add_fact(
                            event.content, session=session, confidence=event.confidence,
                        )
                    elif event.type == "decision":
                        self._brain.add_decision(
                            event.content, session=session, confidence=event.confidence,
                        )
                    elif event.type == "inference":
                        self._brain.add_inference(
                            event.content, session=session, confidence=event.confidence,
                        )
                    elif event.type == "skill":
                        self._brain.add_skill(
                            event.content, session=session, confidence=event.confidence,
                        )
                except Exception as e:
                    logger.warning("Failed to store event: %s (%s)", event.content, e)

            # Handle corrections
            for correction in result.corrections:
                try:
                    old_content = correction.get("old_content", "")
                    new_content = correction.get("new_content", "")
                    if old_content and new_content:
                        # Try to find the old fact
                        facts = self._brain.facts(limit=50)
                        for fact in facts:
                            if old_content.lower() in fact.content.lower():
                                self._brain.add_correction(
                                    new_content,
                                    session=session,
                                    supersedes=fact.id,
                                )
                                break
                except Exception as e:
                    logger.warning("Failed to store correction: %s", e)

        except Exception as e:
            logger.warning("Event extraction failed: %s", e)
            self._last_extraction = None
