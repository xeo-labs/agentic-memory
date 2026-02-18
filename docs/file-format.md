# .amem File Format Specification

This document describes the binary layout of `.amem` files -- the on-disk format for AgenticMemory brain data. The format is designed for fast random access, memory-mapped I/O, and compact storage.

## Design Goals

- **Zero-copy reads:** Node and edge records are fixed-size, enabling direct memory access without deserialization.
- **Memory-mapped friendly:** The layout is aligned for `mmap()` access. The OS handles paging, so brain files larger than available RAM work efficiently.
- **Compact:** Content is LZ4-compressed. Feature vectors are stored as contiguous float arrays. No JSON overhead, no field names repeated per record.
- **Atomic writes:** Writes update the header last. A crash during a write leaves the previous valid state intact.
- **Forward compatible:** The version field and reserved header bytes allow format evolution without breaking existing readers.

## Overview

An `.amem` file consists of six contiguous sections:

```
+------------------+
|   Magic + Header |  64 bytes
+------------------+
|   Node Records   |  node_count * 64 bytes
+------------------+
|   Edge Records   |  edge_count * 13 bytes
+------------------+
|   Content Block  |  variable (LZ4 compressed)
+------------------+
|  Feature Vectors |  node_count * dimension * 4 bytes
+------------------+
|     Indexes      |  variable
+------------------+
```

## Section 1: Header (64 bytes)

The header occupies the first 64 bytes of the file.

| Offset | Size | Type | Field | Description |
|--------|------|------|-------|-------------|
| 0 | 4 | `[u8; 4]` | `magic` | Magic bytes: `0x41 0x4D 0x45 0x4D` (ASCII "AMEM"). |
| 4 | 2 | `u16` | `version` | Format version. Current: `1`. |
| 6 | 2 | `u16` | `flags` | Bitfield. Bit 0: has vectors. Bit 1: has indexes. Bit 2: content compressed. |
| 8 | 4 | `u32` | `node_count` | Total number of node records. |
| 12 | 4 | `u32` | `edge_count` | Total number of edge records. |
| 16 | 2 | `u16` | `dimension` | Feature vector dimension. Default: 128. |
| 18 | 2 | `u16` | `session_count` | Number of distinct sessions. |
| 20 | 8 | `u64` | `content_offset` | Byte offset to the start of the content block. |
| 28 | 8 | `u64` | `content_length` | Length of the content block in bytes (compressed). |
| 36 | 8 | `u64` | `vector_offset` | Byte offset to the start of the feature vector block. |
| 44 | 8 | `u64` | `index_offset` | Byte offset to the start of the index block. |
| 52 | 4 | `u32` | `content_uncompressed` | Uncompressed size of the content block. |
| 56 | 8 | `[u8; 8]` | `reserved` | Reserved for future use. Must be zero. |

**Validation rules:**
- `magic` must be `0x414D454D`.
- `version` must be `<= 1` for this specification.
- `node_count` and `edge_count` must be consistent with file size.
- `dimension` must be a positive integer (typically 128).

## Section 2: Node Records

Immediately following the header. Each node record is a fixed 64 bytes.

| Offset | Size | Type | Field | Description |
|--------|------|------|-------|-------------|
| 0 | 1 | `u8` | `event_type` | Event type enum: 0=Fact, 1=Decision, 2=Inference, 3=Correction, 4=Skill, 5=Episode. |
| 1 | 3 | `[u8; 3]` | `padding` | Alignment padding. Must be zero. |
| 4 | 4 | `u32` | `session` | Session ID. |
| 8 | 4 | `f32` | `confidence` | Confidence score (IEEE 754 single-precision). |
| 12 | 8 | `i64` | `timestamp` | Unix timestamp in seconds (UTC). |
| 20 | 8 | `u64` | `content_offset` | Offset within the decompressed content block where this node's content starts. |
| 28 | 4 | `u32` | `content_length` | Length of this node's content in bytes (decompressed). |
| 32 | 8 | `u64` | `vector_offset` | Offset within the vector block. Set to `u64::MAX` if no vector is present. |
| 40 | 8 | `u64` | `metadata_offset` | Offset within the content block for JSON-encoded metadata. `u64::MAX` if no metadata. |
| 48 | 4 | `u32` | `metadata_length` | Length of metadata in bytes. 0 if no metadata. |
| 52 | 12 | `[u8; 12]` | `reserved` | Reserved. Must be zero. |

**Total:** 64 bytes per node.

**Notes:**
- Node IDs are implicit -- node N is at offset `64 + (N * 64)` from the start of the file.
- `content_offset` and `metadata_offset` are offsets into the **decompressed** content block, not the raw file.
- `vector_offset` is a byte offset into the vector block section.

## Section 3: Edge Records

Immediately following the node records. Each edge record is a fixed 13 bytes.

| Offset | Size | Type | Field | Description |
|--------|------|------|-------|-------------|
| 0 | 4 | `u32` | `source` | Source node ID. |
| 4 | 4 | `u32` | `target` | Target node ID. |
| 8 | 1 | `u8` | `edge_type` | Edge type enum: 0=CausedBy, 1=Supports, 2=Contradicts, 3=Supersedes, 4=RelatedTo, 5=PartOf, 6=TemporalNext. |
| 9 | 4 | `f32` | `weight` | Edge weight (IEEE 754 single-precision). |

**Total:** 13 bytes per edge.

**Notes:**
- Edges are sorted by source node ID for efficient adjacency lookups.
- Both `source` and `target` must be valid node IDs (less than `node_count`).

## Section 4: Content Block

A single LZ4-compressed block containing all node content and metadata strings, concatenated end-to-end.

**Structure (after decompression):**

```
[node_0_content][node_1_content]...[node_N_content][node_0_metadata][node_1_metadata]...
```

All content is UTF-8 encoded. Metadata is JSON-encoded UTF-8 (a flat object with string keys and string values).

**Compression:**
- Algorithm: LZ4 (frame format).
- The header field `content_length` stores the compressed size.
- The header field `content_uncompressed` stores the decompressed size.
- If the `flags` bit 2 is not set, the content block is stored uncompressed (for very small brains where compression overhead is not worthwhile).

**Rationale for LZ4:** LZ4 decompression runs at memory bandwidth speeds (typically 3--5 GB/s), making it effectively free compared to I/O. The compression ratio for natural language text is typically 2--3x.

## Section 5: Feature Vectors

A contiguous block of IEEE 754 single-precision floating-point values. Each node's vector is `dimension` floats (default: 128 floats = 512 bytes per vector).

**Layout:**

```
[node_0_vector: f32 * dimension][node_1_vector: f32 * dimension]...[node_N_vector]
```

**Notes:**
- Vectors are stored in node ID order.
- If a node has no vector (vector_offset is `u64::MAX`), the corresponding slot contains all zeros.
- The contiguous layout is critical for SIMD-accelerated similarity search -- the CPU can scan vectors without pointer chasing.
- Total size: `node_count * dimension * 4` bytes.

## Section 6: Indexes

The index section contains auxiliary data structures for accelerating queries. It is present only if the `flags` bit 1 is set.

### Type Index (Bitmap)

A bitmap index that maps event types to node IDs. Each event type has a bitset of `node_count` bits, where bit N is set if node N has that event type.

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0 | 4 | `u32` | `index_type`: 0x01 (type index). |
| 4 | 4 | `u32` | `num_types`: Number of event types (6). |
| 8 | varies | `[u8; ceil(node_count / 8)] * num_types` | Packed bitsets, one per event type. |

### Session Index

Maps session IDs to their constituent node ID ranges.

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0 | 4 | `u32` | `index_type`: 0x02 (session index). |
| 4 | 4 | `u32` | `num_sessions`: Number of sessions. |
| 8 | varies | `[(session_id: u32, start_node: u32, end_node: u32)] * num_sessions` | Session-to-node-range mapping. |

### Time Index

A sorted array of (timestamp, node_id) pairs for efficient time-range queries.

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0 | 4 | `u32` | `index_type`: 0x03 (time index). |
| 4 | 4 | `u32` | `num_entries`: Number of entries. |
| 8 | varies | `[(timestamp: i64, node_id: u32)] * num_entries` | Sorted by timestamp ascending. |

### Cluster Map

Pre-computed cluster assignments for approximate nearest-neighbor search. Nodes are grouped into clusters based on their feature vectors.

| Offset | Size | Type | Description |
|--------|------|------|-------------|
| 0 | 4 | `u32` | `index_type`: 0x04 (cluster map). |
| 4 | 4 | `u32` | `num_clusters`: Number of clusters. |
| 8 | 4 | `u32` | `dimension`: Vector dimension. |
| 12 | varies | `[f32; dimension] * num_clusters` | Cluster centroid vectors. |
| varies | varies | `[(cluster_id: u32, node_id: u32)] * node_count` | Node-to-cluster assignments, sorted by cluster_id. |

## Version Compatibility

### Version 1 (Current)

The initial release format as described in this document.

**Readers must:**
- Reject files where `magic != 0x414D454D`.
- Reject files where `version > 1`.
- Handle the absence of vectors (flags bit 0 unset) gracefully.
- Handle the absence of indexes (flags bit 1 unset) by falling back to linear scans.

### Forward Compatibility

Future versions may:
- Add new event types (values >= 6 in the `event_type` field). Old readers should treat unknown types as opaque.
- Add new edge types (values >= 7 in the `edge_type` field). Old readers should treat unknown types as opaque.
- Add new index types. Old readers should skip unknown index types.
- Increase the header size. Old readers should use `content_offset` to find the content block rather than hardcoding offsets.
- Utilize the reserved header bytes for new fields.

### Byte Order

All multi-byte integers and floats are stored in **little-endian** byte order.

## Size Estimates

For a brain with N nodes, M edges, and 128-dimensional vectors:

| Component | Size |
|-----------|------|
| Header | 64 bytes |
| Node records | N * 64 bytes |
| Edge records | M * 13 bytes |
| Content block | ~40% of raw content size (LZ4) |
| Feature vectors | N * 512 bytes |
| Indexes | ~N * 20 bytes (approximate) |

**Example:** A brain with 100,000 nodes, 500,000 edges, and average content of 200 bytes per node:

| Component | Size |
|-----------|------|
| Header | 64 B |
| Node records | 6.1 MB |
| Edge records | 6.2 MB |
| Content block | ~8 MB (20 MB raw, ~2.5x compression) |
| Feature vectors | 48.8 MB |
| Indexes | ~1.9 MB |
| **Total** | **~71 MB** |
