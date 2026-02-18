# Benchmarks

Performance measurements for AgenticMemory's core operations across various graph sizes. All benchmarks use the Rust engine directly; Python SDK overhead is negligible for I/O-bound operations and adds approximately 5-15 microseconds per FFI call for compute-bound operations.

## Test Environment

| Parameter | Value |
|-----------|-------|
| Hardware | Apple M-series (ARM64), 16 GB unified memory |
| OS | macOS (Darwin) |
| Rust | 1.75+ (release profile, LTO enabled) |
| Benchmark framework | `criterion.rs` 0.5 |
| Iterations | 100 per measurement (minimum), with statistical warm-up |
| Feature vectors | 128-dimensional, f32 |

All benchmarks are run with `cargo bench` using release-mode compilation with link-time optimization. Results represent the median of 100 iterations after warm-up, with 95% confidence intervals.

## Summary Results

Headline numbers measured at 10K nodes, 50K edges:

| Operation | Median | Description |
|-----------|--------|-------------|
| Add node | 276 ns | Insert a single node into the in-memory graph |
| Add edge | 1.2 ms | Insert an edge with adjacency list update |
| Traverse (depth 5) | 3.4 ms | BFS traversal from a single node, depth limit 5 |
| Similarity search (top-10) | 9 ms | Brute-force cosine similarity across all vectors |
| File write | 32.6 ms | Serialize 10K nodes + 50K edges to .amem with LZ4 |
| File read | 3.7 ms | Deserialize the same file into memory |

## Detailed Results by Graph Size

### Add Node

Time to insert a single cognitive event (fact with 200-character content and metadata) into the in-memory graph.

| Graph Size | Median | Std Dev | Notes |
|------------|--------|---------|-------|
| 1K nodes | 248 ns | 12 ns | Cache-hot, all data fits in L1 |
| 10K nodes | 276 ns | 18 ns | Marginal increase from hash map growth |
| 100K nodes | 312 ns | 25 ns | Occasional hash map resize amortized |

Node insertion is O(1) amortized. Performance is dominated by the hash map insertion for the node ID and the timestamp syscall.

### Add Edge

Time to insert a single edge and update both source and target adjacency lists.

| Graph Size | Median | Std Dev | Notes |
|------------|--------|---------|-------|
| 1K nodes, 5K edges | 0.9 ms | 0.08 ms | Adjacency lists are small |
| 10K nodes, 50K edges | 1.2 ms | 0.11 ms | Average 5 edges per node |
| 100K nodes, 500K edges | 1.8 ms | 0.15 ms | Adjacency list growth |

Edge insertion is O(1) amortized for the edge itself, with O(degree) for adjacency list management. The higher absolute time compared to node insertion comes from updating two adjacency lists and validating both endpoints.

### Graph Traversal (BFS)

Time for a breadth-first traversal from a single starting node. Graph has average degree 10 (5 outgoing, 5 incoming edges per node).

| Graph Size | Depth 3 | Depth 5 | Depth 7 |
|------------|---------|---------|---------|
| 1K nodes | 0.4 ms | 0.8 ms | 1.1 ms |
| 10K nodes | 1.2 ms | 3.4 ms | 8.7 ms |
| 100K nodes | 3.1 ms | 12.4 ms | 45.2 ms |

Traversal time depends on the number of nodes visited, which grows exponentially with depth (bounded by graph size and degree). The visited-set check (hash set lookup) is the primary cost per node.

### Similarity Search

Brute-force cosine similarity search across all 128-dimensional feature vectors. Returns top-k results.

| Graph Size | Top-5 | Top-10 | Top-50 |
|------------|-------|--------|--------|
| 1K nodes | 0.9 ms | 1.0 ms | 1.1 ms |
| 10K nodes | 8.2 ms | 9.0 ms | 9.4 ms |
| 100K nodes | 82 ms | 84 ms | 87 ms |

Similarity search is O(N * D) where N is node count and D is vector dimension. The contiguous vector layout enables SIMD auto-vectorization -- the compiler generates NEON instructions on ARM that process 4 floats per cycle. The top-k selection adds minimal overhead (binary heap, O(N log k)).

At 100K nodes, the cluster map index (when enabled) reduces search to approximately 15-20 ms by scanning only relevant clusters.

### File Write

Time to serialize the complete graph to a `.amem` file, including LZ4 content compression and vector block construction.

| Graph Size | Median | File Size | Throughput |
|------------|--------|-----------|------------|
| 1K nodes, 5K edges | 4.1 ms | 0.7 MB | 170 MB/s |
| 10K nodes, 50K edges | 32.6 ms | 7.1 MB | 218 MB/s |
| 100K nodes, 500K edges | 310 ms | 71 MB | 229 MB/s |

Write time is dominated by LZ4 compression of the content block and sequential writes. The format's sequential layout (no random seeks during write) maximizes throughput.

### File Read

Time to deserialize a `.amem` file into the in-memory graph, including LZ4 decompression.

| Graph Size | Median | File Size | Throughput |
|------------|--------|-----------|------------|
| 1K nodes | 0.5 ms | 0.7 MB | 1.4 GB/s |
| 10K nodes | 3.7 ms | 7.1 MB | 1.9 GB/s |
| 100K nodes | 34 ms | 71 MB | 2.1 GB/s |

Read performance benefits from LZ4's fast decompression (>3 GB/s) and the sequential file layout. Throughput increases at larger sizes because the fixed overhead (header parsing, allocation) is amortized.

### Memory-Mapped Read (MmapReader)

Time to open a file and access a single random node via memory-mapped I/O.

| Graph Size | Open | Random Node Access | Notes |
|------------|------|--------------------|-------|
| 1K nodes | 0.1 ms | 0.3 us | Entire file in page cache |
| 10K nodes | 0.1 ms | 0.4 us | Entire file in page cache |
| 100K nodes | 0.2 ms | 0.5 us | May trigger page fault |

Memory-mapped access avoids reading the entire file upfront. Node access is a direct pointer dereference after the initial page fault. This is ideal for applications that read a small subset of nodes from a large brain.

## Comparison Context

These benchmarks are intended for understanding AgenticMemory's performance profile. Direct comparisons with other systems require careful methodology because they solve different problems.

**Key architectural differences from vector databases:**
- AgenticMemory is an embedded library, not a client-server system. There is no network round-trip.
- The graph structure (edges, traversal) is not present in pure vector databases.
- The binary file format is optimized for single-writer workloads, not concurrent multi-writer access.

**Key architectural differences from graph databases (Neo4j, etc.):**
- AgenticMemory is a file, not a server. No query language parsing, no transaction management.
- The fixed-size node records and contiguous layout are more cache-friendly than pointer-heavy graph representations.
- The trade-off is less flexibility: no ad-hoc queries, no schema migrations, no ACID transactions.

## Reproducing Benchmarks

### Prerequisites

```bash
# Rust toolchain
rustup update stable

# Clone and build
git clone https://github.com/anthropic/agentic-memory.git
cd agentic-memory
```

### Running All Benchmarks

```bash
cargo bench
```

Results are written to `target/criterion/` with HTML reports including statistical analysis, throughput charts, and regression detection.

### Running Specific Benchmarks

```bash
# Only node operations
cargo bench -- add_node

# Only I/O benchmarks
cargo bench -- file_write
cargo bench -- file_read

# Only search benchmarks
cargo bench -- similarity_search
```

### Generating a Report

```bash
cargo bench -- --save-baseline my_hardware
```

The HTML report at `target/criterion/report/index.html` includes:
- Distribution plots for each benchmark
- Mean, median, and standard deviation
- Throughput calculations
- Change detection vs. previous runs

### Custom Graph Sizes

The benchmarks accept environment variables to control graph size:

```bash
BENCH_NODES=50000 BENCH_EDGES=250000 cargo bench
```

### Profiling

For detailed profiling, use `cargo-flamegraph`:

```bash
cargo install flamegraph
cargo flamegraph --bench core_benchmarks -- --bench
```

This generates an SVG flamegraph showing where time is spent during benchmark execution.
