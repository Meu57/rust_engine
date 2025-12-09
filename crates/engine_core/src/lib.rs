// crates/engine_core/src/lib.rs
#![allow(dead_code)]

// Logic Modules
pub mod app;
pub mod input;
pub mod inspector; // <--- New Module
pub mod host;   // <--- NEW
pub mod scene;  // <--- NEW
pub mod engine_loop;
pub mod platform_runner;

// Internal Implementation Modules

mod renderer;
pub mod gui;            // <--- NEW
pub mod plugin_manager; // <--- NEW

// Re-export App so the Editor crate can find it easily
pub use app::App;