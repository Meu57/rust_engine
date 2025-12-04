// crates/engine_shared/src/lib.rs
#![allow(dead_code)]

pub const ENGINE_API_VERSION: u32 = 1;
// Logic Modules
pub mod components;
pub mod input_types; // <--- The new name
pub mod plugin_api;

// Re-exports
pub use components::*;
pub use input_types::*;
pub use plugin_api::*;