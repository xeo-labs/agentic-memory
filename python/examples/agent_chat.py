"""MemoryAgent example — chat with persistent memory.

Requires an LLM provider to be configured.

Run: ANTHROPIC_API_KEY=sk-ant-... python examples/agent_chat.py
"""

import os
import sys

from agentic_memory import Brain, MemoryAgent
from agentic_memory.integrations.base import ChatMessage

# Choose provider based on available API key
provider = None
try:
    if os.environ.get("ANTHROPIC_API_KEY"):
        from agentic_memory.integrations import AnthropicProvider
        if AnthropicProvider is not None:
            provider = AnthropicProvider()
            print("Using Anthropic provider")
    elif os.environ.get("OPENAI_API_KEY"):
        from agentic_memory.integrations import OpenAIProvider
        if OpenAIProvider is not None:
            provider = OpenAIProvider()
            print("Using OpenAI provider")
except Exception as e:
    print(f"Failed to initialize provider: {e}")

if provider is None:
    print("No LLM provider available. Set ANTHROPIC_API_KEY or OPENAI_API_KEY.")
    print("This example requires a real LLM provider.")
    sys.exit(1)

# Create brain and agent
brain = Brain("agent_demo.amem")
agent = MemoryAgent(brain=brain, provider=provider)

# Simulate a multi-session conversation
print("\n=== Session 1 ===")
response = agent.chat("My name is Marcus and I'm a Python developer", session=1)
print(f"Agent: {response.content}")

if agent.last_extraction:
    print(f"\nExtracted {len(agent.last_extraction.events)} events:")
    for event in agent.last_extraction.events:
        print(f"  [{event.type.value}] {event.content}")

print("\n=== Session 2 (new conversation) ===")
response = agent.chat("What do you remember about me?", session=2)
print(f"Agent: {response.content}")

# Show brain state
info = brain.info()
print(f"\nBrain: {info.node_count} nodes, {info.session_count} sessions")
