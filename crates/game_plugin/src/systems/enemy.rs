// crates/game_plugin/src/systems/enemy.rs

//! Enemy spawning for the plugin.
//! This version is FFI-safe: the plugin does *not* mutate the host World directly.
//! Instead it calls the host-provided `spawn_fn(world_ptr, x, y)` to request spawns.

use std::ffi::c_void;

/// Spawn enemies by calling back into the host.
///
/// - `spawn_fn` : extern "C" fn(*mut c_void, f32, f32) provided by the host.
/// - `world_ptr` : opaque pointer to host World (plugin must not dereference it).
/// - `timer`     : spawn timer (mutable reference owned by plugin instance).
/// - `dt`        : delta time this frame.
///
/// NOTE: The actual allocation / ECS mutation happens inside the host implementation
/// of `spawn_fn`. The plugin only computes when/where to spawn and requests it.
pub fn spawn_enemies(
    spawn_fn: extern "C" fn(*mut c_void, f32, f32),
    world_ptr: *mut c_void,
    timer: &mut f32,
    dt: f32,
) {
    // Decrement timer
    *timer -= dt;

    if *timer <= 0.0 {
        // reset timer (example cadence)
        *timer = 2.0;

        // pseudo-random position based on dt (placeholder)
        let rx = (dt * 12345.0).rem_euclid(1280.0);
        let ry = (dt * 67890.0).rem_euclid(720.0);

        // Safety: plugin must not dereference world_ptr.
        // The host implementation of `spawn_fn` will cast it back to &mut World and mutate safely.
        spawn_fn(world_ptr, rx, ry);
    }
}
