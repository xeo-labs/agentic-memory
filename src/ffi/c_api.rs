//! C-compatible FFI bindings.

use std::ffi::CStr;
use std::os::raw::c_char;
use std::path::Path;

use crate::format::{AmemReader, AmemWriter};
use crate::graph::MemoryGraph;
use crate::types::{CognitiveEventBuilder, Edge, EdgeType, EventType};

const AMEM_OK: i32 = 0;
const AMEM_ERR_IO: i32 = -1;
const AMEM_ERR_INVALID: i32 = -2;
const AMEM_ERR_NOT_FOUND: i32 = -3;
const AMEM_ERR_OVERFLOW: i32 = -4;
const AMEM_ERR_NULL_PTR: i32 = -5;

/// Create a new empty graph. Returns handle or NULL on failure.
#[no_mangle]
pub extern "C" fn amem_graph_new(dimension: u32) -> *mut std::ffi::c_void {
    std::panic::catch_unwind(|| {
        let graph = Box::new(MemoryGraph::new(dimension as usize));
        Box::into_raw(graph) as *mut std::ffi::c_void
    })
    .unwrap_or(std::ptr::null_mut())
}

/// Load a graph from an .amem file. Returns handle or NULL on failure.
///
/// # Safety
///
/// The caller must ensure that all pointer arguments are valid and non-null
/// (unless the function explicitly handles null pointers), and that any
/// pointed-to data is valid for the expected type and lifetime.
#[no_mangle]
pub unsafe extern "C" fn amem_graph_open(path: *const c_char) -> *mut std::ffi::c_void {
    std::panic::catch_unwind(|| {
        if path.is_null() {
            return std::ptr::null_mut();
        }
        let path_str = unsafe { CStr::from_ptr(path) };
        let path_str = match path_str.to_str() {
            Ok(s) => s,
            Err(_) => return std::ptr::null_mut(),
        };
        match AmemReader::read_from_file(Path::new(path_str)) {
            Ok(graph) => Box::into_raw(Box::new(graph)) as *mut std::ffi::c_void,
            Err(_) => std::ptr::null_mut(),
        }
    })
    .unwrap_or(std::ptr::null_mut())
}

/// Save a graph to an .amem file. Returns AMEM_OK or error code.
///
/// # Safety
///
/// The caller must ensure that all pointer arguments are valid and non-null
/// (unless the function explicitly handles null pointers), and that any
/// pointed-to data is valid for the expected type and lifetime.
#[no_mangle]
pub unsafe extern "C" fn amem_graph_save(graph: *mut std::ffi::c_void, path: *const c_char) -> i32 {
    std::panic::catch_unwind(|| {
        if graph.is_null() || path.is_null() {
            return AMEM_ERR_NULL_PTR;
        }
        let graph = unsafe { &*(graph as *const MemoryGraph) };
        let path_str = unsafe { CStr::from_ptr(path) };
        let path_str = match path_str.to_str() {
            Ok(s) => s,
            Err(_) => return AMEM_ERR_INVALID,
        };
        let writer = AmemWriter::new(graph.dimension());
        match writer.write_to_file(graph, Path::new(path_str)) {
            Ok(()) => AMEM_OK,
            Err(_) => AMEM_ERR_IO,
        }
    })
    .unwrap_or(AMEM_ERR_IO)
}

/// Free a graph handle.
///
/// # Safety
///
/// The caller must ensure that all pointer arguments are valid and non-null
/// (unless the function explicitly handles null pointers), and that any
/// pointed-to data is valid for the expected type and lifetime.
#[no_mangle]
pub unsafe extern "C" fn amem_graph_free(graph: *mut std::ffi::c_void) {
    if !graph.is_null() {
        let _ = std::panic::catch_unwind(|| unsafe {
            drop(Box::from_raw(graph as *mut MemoryGraph));
        });
    }
}

/// Get node count.
///
/// # Safety
///
/// The caller must ensure that all pointer arguments are valid and non-null
/// (unless the function explicitly handles null pointers), and that any
/// pointed-to data is valid for the expected type and lifetime.
#[no_mangle]
pub unsafe extern "C" fn amem_graph_node_count(graph: *mut std::ffi::c_void) -> u64 {
    std::panic::catch_unwind(|| {
        if graph.is_null() {
            return 0;
        }
        let graph = unsafe { &*(graph as *const MemoryGraph) };
        graph.node_count() as u64
    })
    .unwrap_or(0)
}

/// Get edge count.
///
/// # Safety
///
/// The caller must ensure that all pointer arguments are valid and non-null
/// (unless the function explicitly handles null pointers), and that any
/// pointed-to data is valid for the expected type and lifetime.
#[no_mangle]
pub unsafe extern "C" fn amem_graph_edge_count(graph: *mut std::ffi::c_void) -> u64 {
    std::panic::catch_unwind(|| {
        if graph.is_null() {
            return 0;
        }
        let graph = unsafe { &*(graph as *const MemoryGraph) };
        graph.edge_count() as u64
    })
    .unwrap_or(0)
}

/// Get dimension.
///
/// # Safety
///
/// The caller must ensure that all pointer arguments are valid and non-null
/// (unless the function explicitly handles null pointers), and that any
/// pointed-to data is valid for the expected type and lifetime.
#[no_mangle]
pub unsafe extern "C" fn amem_graph_dimension(graph: *mut std::ffi::c_void) -> u32 {
    std::panic::catch_unwind(|| {
        if graph.is_null() {
            return 0;
        }
        let graph = unsafe { &*(graph as *const MemoryGraph) };
        graph.dimension() as u32
    })
    .unwrap_or(0)
}

/// Add a node. Returns the assigned node ID, or -1 on error.
///
/// # Safety
///
/// The caller must ensure that all pointer arguments are valid and non-null
/// (unless the function explicitly handles null pointers), and that any
/// pointed-to data is valid for the expected type and lifetime.
#[no_mangle]
pub unsafe extern "C" fn amem_graph_add_node(
    graph: *mut std::ffi::c_void,
    event_type: u8,
    content: *const c_char,
    session_id: u32,
    confidence: f32,
) -> i64 {
    std::panic::catch_unwind(|| {
        if graph.is_null() || content.is_null() {
            return -1i64;
        }
        let graph = unsafe { &mut *(graph as *mut MemoryGraph) };
        let content_str = unsafe { CStr::from_ptr(content) };
        let content_str = match content_str.to_str() {
            Ok(s) => s,
            Err(_) => return -1,
        };
        let et = match EventType::from_u8(event_type) {
            Some(et) => et,
            None => return -1,
        };
        let event = CognitiveEventBuilder::new(et, content_str)
            .session_id(session_id)
            .confidence(confidence)
            .build();
        match graph.add_node(event) {
            Ok(id) => id as i64,
            Err(_) => -1,
        }
    })
    .unwrap_or(-1)
}

/// Get a node's content. Writes to buffer. Returns content length or error.
///
/// # Safety
///
/// The caller must ensure that all pointer arguments are valid and non-null
/// (unless the function explicitly handles null pointers), and that any
/// pointed-to data is valid for the expected type and lifetime.
#[no_mangle]
pub unsafe extern "C" fn amem_graph_get_content(
    graph: *mut std::ffi::c_void,
    node_id: u64,
    buffer: *mut c_char,
    buffer_size: u32,
) -> i32 {
    std::panic::catch_unwind(|| {
        if graph.is_null() || buffer.is_null() {
            return AMEM_ERR_NULL_PTR;
        }
        let graph = unsafe { &*(graph as *const MemoryGraph) };
        match graph.get_node(node_id) {
            Some(node) => {
                let content_bytes = node.content.as_bytes();
                if content_bytes.len() + 1 > buffer_size as usize {
                    return AMEM_ERR_OVERFLOW;
                }
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        content_bytes.as_ptr(),
                        buffer as *mut u8,
                        content_bytes.len(),
                    );
                    *buffer.add(content_bytes.len()) = 0; // null terminator
                }
                content_bytes.len() as i32
            }
            None => AMEM_ERR_NOT_FOUND,
        }
    })
    .unwrap_or(AMEM_ERR_INVALID)
}

/// Get a node's confidence. Returns -1.0 if not found.
///
/// # Safety
///
/// The caller must ensure that all pointer arguments are valid and non-null
/// (unless the function explicitly handles null pointers), and that any
/// pointed-to data is valid for the expected type and lifetime.
#[no_mangle]
pub unsafe extern "C" fn amem_graph_get_confidence(
    graph: *mut std::ffi::c_void,
    node_id: u64,
) -> f32 {
    std::panic::catch_unwind(|| {
        if graph.is_null() {
            return -1.0;
        }
        let graph = unsafe { &*(graph as *const MemoryGraph) };
        graph
            .get_node(node_id)
            .map(|n| n.confidence)
            .unwrap_or(-1.0)
    })
    .unwrap_or(-1.0)
}

/// Get a node's event type. Returns -1 if not found.
///
/// # Safety
///
/// The caller must ensure that all pointer arguments are valid and non-null
/// (unless the function explicitly handles null pointers), and that any
/// pointed-to data is valid for the expected type and lifetime.
#[no_mangle]
pub unsafe extern "C" fn amem_graph_get_event_type(
    graph: *mut std::ffi::c_void,
    node_id: u64,
) -> i32 {
    std::panic::catch_unwind(|| {
        if graph.is_null() {
            return -1;
        }
        let graph = unsafe { &*(graph as *const MemoryGraph) };
        graph
            .get_node(node_id)
            .map(|n| n.event_type as i32)
            .unwrap_or(-1)
    })
    .unwrap_or(-1)
}

/// Add an edge. Returns AMEM_OK or error code.
///
/// # Safety
///
/// The caller must ensure that all pointer arguments are valid and non-null
/// (unless the function explicitly handles null pointers), and that any
/// pointed-to data is valid for the expected type and lifetime.
#[no_mangle]
pub unsafe extern "C" fn amem_graph_add_edge(
    graph: *mut std::ffi::c_void,
    source_id: u64,
    target_id: u64,
    edge_type: u8,
    weight: f32,
) -> i32 {
    std::panic::catch_unwind(|| {
        if graph.is_null() {
            return AMEM_ERR_NULL_PTR;
        }
        let graph = unsafe { &mut *(graph as *mut MemoryGraph) };
        let et = match EdgeType::from_u8(edge_type) {
            Some(et) => et,
            None => return AMEM_ERR_INVALID,
        };
        let edge = Edge::new(source_id, target_id, et, weight);
        match graph.add_edge(edge) {
            Ok(()) => AMEM_OK,
            Err(_) => AMEM_ERR_INVALID,
        }
    })
    .unwrap_or(AMEM_ERR_INVALID)
}

/// Get edges from a node. Returns edge count or error.
///
/// # Safety
///
/// The caller must ensure that all pointer arguments are valid and non-null
/// (unless the function explicitly handles null pointers), and that any
/// pointed-to data is valid for the expected type and lifetime.
#[no_mangle]
pub unsafe extern "C" fn amem_graph_get_edges(
    graph: *mut std::ffi::c_void,
    source_id: u64,
    target_ids: *mut u64,
    edge_types: *mut u8,
    weights: *mut f32,
    max_edges: u32,
) -> i32 {
    std::panic::catch_unwind(|| {
        if graph.is_null() || target_ids.is_null() || edge_types.is_null() || weights.is_null() {
            return AMEM_ERR_NULL_PTR;
        }
        let graph = unsafe { &*(graph as *const MemoryGraph) };
        let edges = graph.edges_from(source_id);
        let count = edges.len().min(max_edges as usize);
        for (i, edge) in edges.iter().take(count).enumerate() {
            unsafe {
                *target_ids.add(i) = edge.target_id;
                *edge_types.add(i) = edge.edge_type as u8;
                *weights.add(i) = edge.weight;
            }
        }
        count as i32
    })
    .unwrap_or(AMEM_ERR_INVALID)
}

/// Traverse from a start node. Returns visited count.
///
/// # Safety
///
/// The caller must ensure that all pointer arguments are valid and non-null
/// (unless the function explicitly handles null pointers), and that any
/// pointed-to data is valid for the expected type and lifetime.
#[no_mangle]
pub unsafe extern "C" fn amem_graph_traverse(
    graph: *mut std::ffi::c_void,
    start_id: u64,
    edge_types_ptr: *const u8,
    edge_type_count: u32,
    direction: u8,
    max_depth: u32,
    visited_ids: *mut u64,
    max_results: u32,
) -> i32 {
    std::panic::catch_unwind(|| {
        if graph.is_null() || edge_types_ptr.is_null() || visited_ids.is_null() {
            return AMEM_ERR_NULL_PTR;
        }
        let graph_ref = unsafe { &*(graph as *const MemoryGraph) };

        let edge_types: Vec<EdgeType> = (0..edge_type_count)
            .filter_map(|i| {
                let val = unsafe { *edge_types_ptr.add(i as usize) };
                EdgeType::from_u8(val)
            })
            .collect();

        let dir = match direction {
            0 => crate::graph::TraversalDirection::Forward,
            1 => crate::graph::TraversalDirection::Backward,
            _ => crate::graph::TraversalDirection::Both,
        };

        let query_engine = crate::engine::QueryEngine::new();
        let params = crate::engine::TraversalParams {
            start_id,
            edge_types,
            direction: dir,
            max_depth,
            max_results: max_results as usize,
            min_confidence: 0.0,
        };

        match query_engine.traverse(graph_ref, params) {
            Ok(result) => {
                let count = result.visited.len().min(max_results as usize);
                for (i, &id) in result.visited.iter().take(count).enumerate() {
                    unsafe {
                        *visited_ids.add(i) = id;
                    }
                }
                count as i32
            }
            Err(_) => AMEM_ERR_NOT_FOUND,
        }
    })
    .unwrap_or(AMEM_ERR_INVALID)
}

/// Resolve: follow SUPERSEDES chain. Returns final node ID or error.
///
/// # Safety
///
/// The caller must ensure that all pointer arguments are valid and non-null
/// (unless the function explicitly handles null pointers), and that any
/// pointed-to data is valid for the expected type and lifetime.
#[no_mangle]
pub unsafe extern "C" fn amem_graph_resolve(graph: *mut std::ffi::c_void, node_id: u64) -> i64 {
    std::panic::catch_unwind(|| {
        if graph.is_null() {
            return -1i64;
        }
        let graph_ref = unsafe { &*(graph as *const MemoryGraph) };
        let query_engine = crate::engine::QueryEngine::new();
        match query_engine.resolve(graph_ref, node_id) {
            Ok(node) => node.id as i64,
            Err(_) => -1,
        }
    })
    .unwrap_or(-1)
}

/// Record a correction. Returns new node ID or error.
///
/// # Safety
///
/// The caller must ensure that all pointer arguments are valid and non-null
/// (unless the function explicitly handles null pointers), and that any
/// pointed-to data is valid for the expected type and lifetime.
#[no_mangle]
pub unsafe extern "C" fn amem_graph_correct(
    graph: *mut std::ffi::c_void,
    old_node_id: u64,
    new_content: *const c_char,
    session_id: u32,
) -> i64 {
    std::panic::catch_unwind(|| {
        if graph.is_null() || new_content.is_null() {
            return -1i64;
        }
        let graph_mut = unsafe { &mut *(graph as *mut MemoryGraph) };
        let content_str = unsafe { CStr::from_ptr(new_content) };
        let content_str = match content_str.to_str() {
            Ok(s) => s,
            Err(_) => return -1,
        };
        let write_engine = crate::engine::WriteEngine::new(graph_mut.dimension());
        match write_engine.correct(graph_mut, old_node_id, content_str, session_id) {
            Ok(id) => id as i64,
            Err(_) => -1,
        }
    })
    .unwrap_or(-1)
}

/// Touch a node (update access tracking).
///
/// # Safety
///
/// The caller must ensure that all pointer arguments are valid and non-null
/// (unless the function explicitly handles null pointers), and that any
/// pointed-to data is valid for the expected type and lifetime.
#[no_mangle]
pub unsafe extern "C" fn amem_graph_touch(graph: *mut std::ffi::c_void, node_id: u64) -> i32 {
    std::panic::catch_unwind(|| {
        if graph.is_null() {
            return AMEM_ERR_NULL_PTR;
        }
        let graph_mut = unsafe { &mut *(graph as *mut MemoryGraph) };
        let write_engine = crate::engine::WriteEngine::new(graph_mut.dimension());
        match write_engine.touch(graph_mut, node_id) {
            Ok(()) => AMEM_OK,
            Err(_) => AMEM_ERR_NOT_FOUND,
        }
    })
    .unwrap_or(AMEM_ERR_INVALID)
}
