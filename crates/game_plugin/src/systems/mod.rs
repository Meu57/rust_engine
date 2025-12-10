// crates/game_plugin/src/systems/mod.rs
pub mod player;
pub mod enemy;
pub mod camera; // <--- NEW MODULE

// --- SHARED SETTINGS ---
// Define the map size once here. Both Player and Camera will use this.
pub const MAP_WIDTH: f32 = 2000.0;
pub const MAP_HEIGHT: f32 = 2000.0;