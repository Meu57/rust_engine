// crates/engine_core/src/input/mod.rs
pub mod registry;
pub mod map;
pub mod arbiter;
pub mod ffi;

// Re-export core types to maintain the API `crate::input::ActionRegistry`
pub use registry::ActionRegistry;
pub use map::InputMap;
pub use arbiter::Arbiter;
pub use ffi::{host_get_action_id, GLOBAL_REGISTRY};