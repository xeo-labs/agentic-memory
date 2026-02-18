"""Cross-provider example — use the same brain with different LLM providers.

Demonstrates that the brain file is provider-agnostic. Any LLM can read
memories written by another LLM.

Run: ANTHROPIC_API_KEY=... OPENAI_API_KEY=... python examples/cross_provider.py
"""

import os
import sys

from agentic_memory import Brain, MemoryAgent

brain = Brain("cross_provider.amem")

# Build up knowledge without any LLM (core Brain API)
print("=== Building knowledge base ===")
brain.add_fact("User's name is Alex", session=1, confidence=0.95)
brain.add_fact("Alex prefers TypeScript", session=1, confidence=0.9)
brain.add_fact("Alex works on a React project", session=1, confidence=0.85)
brain.add_decision("Recommended Next.js for their SSR needs", session=1)

info = brain.info()
print(f"Brain has {info.node_count} nodes across {info.session_count} sessions")

# Now use different providers to query the same brain
providers = []

if os.environ.get("ANTHROPIC_API_KEY"):
    try:
        from agentic_memory.integrations import AnthropicProvider
        if AnthropicProvider is not None:
            providers.append(("Anthropic", AnthropicProvider()))
    except Exception as e:
        print(f"Anthropic init failed: {e}")

if os.environ.get("OPENAI_API_KEY"):
    try:
        from agentic_memory.integrations import OpenAIProvider
        if OpenAIProvider is not None:
            providers.append(("OpenAI", OpenAIProvider()))
    except Exception as e:
        print(f"OpenAI init failed: {e}")

if not providers:
    print("\nNo LLM providers configured. Set API keys to test cross-provider.")
    print("The brain file works with any provider — that's the point!")
    sys.exit(0)

for name, provider in providers:
    print(f"\n=== Querying with {name} ===")
    agent = MemoryAgent(brain=brain, provider=provider, extract_events=False)
    response = agent.chat("What do you know about me?", session=2)
    print(f"{name}: {response.content[:200]}...")
