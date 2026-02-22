# Frequently Asked Questions

## How is this different from a vector database?

Vector databases (Pinecone, Weaviate, Qdrant, Chroma, etc.) solve one problem: find the most similar vectors to a query. They are retrieval systems. You put embeddings in, you get nearest neighbors out.

AgenticMemory is a **cognitive graph** -- it stores typed knowledge (facts, decisions, inferences, corrections, skills, episodes) and the **relationships** between them (causal, supportive, contradictory, temporal). It happens to support vector similarity search as one of several query mechanisms, but that is not its primary purpose.

The key differences:

- **Typed nodes.** A vector database treats all entries as equivalent blobs with embeddings. AgenticMemory distinguishes between a fact and a decision, a skill and an episode. This typing enables structured queries ("show me all decisions from session 3") that vector databases cannot express.

- **Edges.** AgenticMemory stores explicit relationships. You can traverse from a decision to the facts that supported it, or from a correction to the fact it superseded. Vector databases have no concept of relationships between entries.

- **Supersedes chains.** When knowledge changes, AgenticMemory creates a correction linked by a `supersedes` edge. You can resolve to the latest version while preserving the full history. In a vector database, you would overwrite the old entry or maintain multiple conflicting entries with no way to express which is current.

- **Embedded, not a server.** AgenticMemory is a library and a file. There is no server to run, no network latency, no deployment complexity. A vector database is typically a separate service.

- **Portability.** A brain is a single `.amem` file. Copy it, email it, commit it to git, back it up. Moving data between vector database instances is significantly more involved.

You might use both in the same system: a vector database for large-scale document retrieval (millions of chunks), and AgenticMemory for the agent's working knowledge (thousands to hundreds of thousands of cognitive events).

## Can I use this with my framework?

Most likely yes. AgenticMemory is designed to be framework-agnostic.

**Direct support:**
- Raw Python (`Brain` class) -- works anywhere Python runs.
- Anthropic Claude -- `AnthropicProvider` built in.
- OpenAI GPT -- `OpenAIProvider` built in.
- Ollama -- `OllamaProvider` built in.
- LangChain -- wrapper integration available.
- CrewAI -- tool integration available.
- Claude Desktop, Claude Code, VS Code, Cursor, Windsurf -- MCP server integration via `agentic-memory-mcp`.

**Easy to integrate with:**
- AutoGen, Agency Swarm, Semantic Kernel, Haystack, LlamaIndex, or any Python-based agent framework. Use the `Brain` class directly or implement a thin adapter.
- Any system that supports MCP (Model Context Protocol) -- install `agentic-memory-mcp` (`cargo install agentic-memory-mcp`) and connect via stdio.

**Building a custom integration:**
If your framework is not listed, you have two options:
1. Use the `Brain` class directly in your framework's memory or tool layer.
2. Implement the `LLMProvider` interface to connect any LLM backend.

See the [Integration Guide](integration-guide.md) for detailed examples.

## What happens when the file gets really big?

AgenticMemory handles large files efficiently through several mechanisms:

**Memory-mapped I/O.** The `MmapReader` uses `mmap()` to access the file without loading it entirely into memory. The operating system pages data in and out as needed. You can work with brain files larger than your available RAM.

**LZ4 compression.** Content is compressed with LZ4, which achieves 2-3x compression on natural language text while decompressing at memory bandwidth speeds (3-5 GB/s). A brain with 100K nodes and average 200-byte content stores roughly 71 MB on disk instead of 300+ MB in JSON.

**Fixed-size records.** Node and edge records are fixed-size (64 bytes and 13 bytes respectively), so accessing node N is a direct offset calculation with no scanning or parsing. This is O(1) regardless of file size.

**Practical limits:**
- 1K nodes: sub-megabyte file, all operations under 1 ms.
- 10K nodes: ~7 MB file, all operations under 35 ms.
- 100K nodes: ~71 MB file, most operations under 100 ms. Similarity search may reach 80-85 ms without indexing.
- 1M nodes: ~700 MB file. Memory-mapped access is strongly recommended. Similarity search benefits from the cluster map index.

If your use case exceeds a million nodes, consider partitioning across multiple brain files (e.g., one per project, one per agent, or one per time period).

## Is it thread-safe?

**Rust core:** The `CognitiveGraph` struct is `Send` but not `Sync`. You can move it between threads but cannot share a mutable reference across threads without external synchronization (e.g., `Mutex<CognitiveGraph>`). The `MmapReader` is both `Send` and `Sync` -- multiple threads can read concurrently.

**Python SDK:** The `Brain` class uses an internal lock for thread safety. Multiple threads can call methods on the same `Brain` instance, but writes are serialized. This is sufficient for most agent architectures where one thread handles conversation and another handles background processing.

**File-level:** The `.amem` file format supports single-writer, multiple-reader access. One process can write while other processes read via `MmapReader`. Concurrent writes from multiple processes to the same file are not supported and will corrupt data.

## Can multiple agents write to the same brain?

**Same process:** Yes. Multiple `MemoryAgent` instances can share a single `Brain` object. Writes are serialized by the internal lock. Each agent should use a different session ID for attribution.

```python
brain = Brain("shared.amem")
agent_a = MemoryAgent(brain, provider_a)
agent_b = MemoryAgent(brain, provider_b)
# Both agents write to the same brain file
```

**Different processes:** Not simultaneously to the same file. The `.amem` format is single-writer. Options for multi-process scenarios:

1. **Separate files, merge later.** Each agent writes to its own brain file. Periodically merge them.
2. **MCP server.** Run `agentic-memory-mcp serve` as a single-writer gateway. Multiple agents connect as MCP clients, and the server serializes writes.
3. **Write lock.** Use OS-level file locking (e.g., `flock`) to ensure only one process writes at a time.

## How do I back up my brain?

A brain is a single `.amem` file. Back it up the same way you would any file:

```bash
# Simple copy
cp agent.amem agent.amem.backup

# With timestamp
cp agent.amem "agent_$(date +%Y%m%d_%H%M%S).amem"

# Git (the file is binary, so git stores full snapshots)
git add agent.amem
git commit -m "Brain snapshot after session 42"

# rsync to remote
rsync agent.amem user@backup-server:/backups/
```

The file is self-contained -- no external dependencies, no database server state, no separate index files. A copied `.amem` file is a complete, valid brain.

For automated backups, copy the file between sessions (when no writes are in progress) or use the `MmapReader` to take a consistent snapshot while the brain is open.

## Does it work on Windows?

Yes. AgenticMemory supports Windows (x86_64) with the following notes:

**Python SDK:** Pre-built wheels are available for Windows via `pip install agentic-brain`. No Rust toolchain needed.

**Rust CLI:** Build from source with `cargo install agentic-memory` or `cargo build --release`. All tests pass on Windows. Memory-mapped I/O uses `CreateFileMapping`/`MapViewOfFile` under the hood.

**Known differences:**
- File paths use backslashes. The SDK handles path normalization, but if you pass paths to the CLI, use Windows-style paths or forward slashes (both work).
- Memory-mapped file size may be limited by the virtual address space on 32-bit Windows. Use 64-bit Windows for brain files larger than ~1 GB.
- The MCP server (`agentic-memory-mcp serve`) works on Windows but requires port configuration if the default port is in use.

The CI pipeline runs the full test suite (440 tests) on Windows, macOS, and Linux for every release.

## How do embeddings work?

Each cognitive event can have a 128-dimensional feature vector (embedding) associated with it. These vectors enable semantic similarity search -- finding events whose meaning is close to a query, even if the exact words differ.

**How vectors are generated:**

1. **Default (built-in).** AgenticMemory includes a lightweight embedding model that runs locally. When you add an event, the content is automatically embedded. No API calls, no external dependencies.

2. **Provider embeddings.** If your `LLMProvider` implements the `embed()` method, the provider's embedding model is used instead. This lets you use OpenAI's `text-embedding-3-small`, Anthropic's embeddings, or any other model.

3. **Manual.** For advanced use cases, you can provide pre-computed vectors directly via the Rust API (`CognitiveNode.vector` field).

**Why 128 dimensions?**

The 128-dim default balances quality and performance:
- Storage: 512 bytes per vector (128 * 4 bytes per f32).
- Search speed: 128 floats fit comfortably in SIMD registers. At 100K nodes, brute-force search takes ~85 ms.
- Quality: 128 dimensions capture semantic relationships effectively for the typical scale of agent memory (thousands to hundreds of thousands of events). Higher dimensions (768, 1536) provide marginal quality improvement at significantly higher storage and search cost.

**How search works:**

Similarity search computes the cosine similarity between the query vector and every stored vector, then returns the top-k results. The vectors are stored contiguously in the `.amem` file, enabling cache-friendly sequential access and SIMD auto-vectorization.

For brains larger than ~50K nodes, the cluster map index (k-means clustering of vectors) accelerates search by scanning only relevant clusters, reducing search time by roughly 4-5x.

## Can I use it without any LLM?

Absolutely. The `Brain` class works entirely without an LLM. You control what gets stored, how events are linked, and how queries are performed.

```python
from agentic_memory import Brain

brain = Brain("manual.amem")

# Store events directly
brain.add_fact("API rate limit is 1000 req/min", confidence=0.99)
brain.add_decision("Implement exponential backoff", confidence=0.9)

# Query
facts = brain.facts()
results = brain.search("rate limiting")
```

The LLM is only needed for:
- **Automatic extraction** (`MemoryAgent.chat()`) -- the LLM identifies what facts, decisions, and inferences are present in a conversation.
- **Automatic linking** (`auto_link=True` in `MemoryAgent`) -- the LLM suggests edges between new events and existing knowledge.

If your application has structured input (API responses, database records, sensor data), you likely do not need an LLM at all. Store events directly and build your own extraction logic.

The `amem` CLI also works entirely without an LLM. The separate MCP server (`agentic-memory-mcp`) does not use an LLM itself -- it just exposes the brain as tools for an external LLM client.

## What about privacy and security?

**Data stays local.** Brain files are stored on your filesystem. The `Brain` class never sends data to any external service. The Rust engine has no network capabilities.

**LLM providers see conversation data.** When using `MemoryAgent` with a cloud provider (Anthropic, OpenAI), conversation text is sent to the provider's API for completion and extraction. The extracted events are then stored locally in the brain file. If privacy is a concern:
- Use `OllamaProvider` with a local model. No data leaves your machine.
- Use the `Brain` class directly without any LLM provider.

**File encryption.** The `.amem` format does not include encryption. If you need encryption at rest, use filesystem-level encryption (FileVault on macOS, BitLocker on Windows, LUKS on Linux) or encrypt the file with a tool like `age` or `gpg` before archiving.

**Access control.** The brain file uses standard filesystem permissions. Set appropriate permissions on the file (`chmod 600 brain.amem` on Unix) to restrict access.

**Sensitive data.** Be mindful of what your agent stores. If conversations contain sensitive information (passwords, API keys, personal data), that information may be extracted and stored in the brain file. Consider:
- Using metadata tags to mark sensitive events.
- Implementing a sanitization step before storage.
- Periodically auditing brain contents with `amem search`.

**MCP server.** When running `agentic-memory-mcp serve`, the server binds to `127.0.0.1` by default (localhost only). It does not expose the brain to the network. If you need remote access, use an SSH tunnel or VPN rather than binding to `0.0.0.0`.
