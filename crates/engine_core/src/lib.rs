// crates/engine_core/src/lib.rs
#![allow(dead_code)]

// Logic Modules
pub mod app;
pub mod input;

// Internal Implementation Modules
mod hot_reload;
mod renderer;

// Re-export App so the Editor crate can find it easily
pub use app::App;