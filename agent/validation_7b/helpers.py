"""Shared utilities for Phase 7B cross-provider validation."""

from __future__ import annotations

from amem_agent.brain import Brain
from amem_agent.llm.base import LLMBackend, Message
from amem_agent.memory.context import build_memory_context, extract_and_store
from amem_agent.agent.prompts import build_full_system_prompt


class CrossProviderSession:
    """A validation session that explicitly tracks its provider for reporting."""

    def __init__(
        self,
        brain_path: str,
        backend: LLMBackend,
        session_id: int,
        provider_name: str,
    ):
        from validation_7b.harness import _find_amem_binary

        self.brain = Brain(brain_path, amem_binary=_find_amem_binary())
        self.brain.ensure_exists()
        self.llm = backend
        self.session_id = session_id
        self.provider_name = provider_name
        self.history: list[Message] = []
        self.events_written = 0

    def send(self, user_message: str) -> str:
        """Send a message, get response, form memories. Returns response text."""
        # 1. Build memory context from brain
        memory_context = build_memory_context(
            brain=self.brain,
            session_id=self.session_id,
            user_message=user_message,
            llm=self.llm,
        )

        # 2. Build prompt
        system_prompt = build_full_system_prompt(memory_context=memory_context)
        messages = [Message(role="system", content=system_prompt)]
        for msg in self.history[-10:]:
            messages.append(msg)
        messages.append(Message(role="user", content=user_message))

        # 3. Get response
        response = self.llm.chat(messages)

        # 4. Update history
        self.history.append(Message(role="user", content=user_message))
        self.history.append(Message(role="assistant", content=response.content))

        # 5. Memory formation
        try:
            extract_and_store(
                brain=self.brain,
                llm=self.llm,
                user_message=user_message,
                assistant_response=response.content,
                session_id=self.session_id,
            )
            self.events_written += 1
        except Exception:
            pass  # Memory formation failure is non-fatal

        return response.content

    def close(self):
        """End session with episode compression."""
        try:
            summary = f"Cross-provider validation session {self.session_id} ({self.provider_name})"
            self.brain.add_episode(summary, self.session_id)
        except Exception:
            pass


# ------------------------------------------------------------------
# Assertion helpers
# ------------------------------------------------------------------

def assert_response_contains(response: str, keywords: list[str]) -> bool:
    """Check if response contains ALL keywords (case-insensitive)."""
    r = response.lower()
    return all(k.lower() in r for k in keywords)


def assert_response_contains_any(response: str, keywords: list[str]) -> bool:
    """Check if response contains ANY keyword (case-insensitive)."""
    r = response.lower()
    return any(k.lower() in r for k in keywords)


def run_with_retry(fn, retries: int = 1) -> bool:
    """Run a test function with retry for LLM non-determinism."""
    for attempt in range(retries + 1):
        try:
            fn()
            return True
        except AssertionError:
            if attempt < retries:
                continue
            raise
        except Exception:
            if attempt < retries:
                continue
            raise
    return False
