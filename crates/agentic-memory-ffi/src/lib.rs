//! Minimal FFI facade for AgenticMemory.

/// Crate version exposed for foreign runtimes.
pub fn agentic_memory_ffi_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
