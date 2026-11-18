//! Directly compiles the production operational-memory module on Windows
//! without depending on a separate, currently failing inference-worker API.
//! This is module acceptance, not whole-app compilation.
#[path = "../../../src-tauri/lu-core/src/operational_memory.rs"]
pub mod operational_memory;
