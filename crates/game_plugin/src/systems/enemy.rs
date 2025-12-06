//! Enemy spawning for the plugin.
//! The plugin does *not* mutate the host World directly.
//! Instead it calls the host-provided `spawn_fn(ctx, x, y)` to request spawns.

use engine_shared::HostContext;

/// Spawn enemies by calling back into the host.
///
/// - `spawn_fn`  : extern "C" fn(*mut HostContext, f32, f32) provided by the host.
/// - `world_ctx` : opaque pointer to host context (actually a World on the host side).
/// - `timer`     : spawn timer (mutable reference owned by plugin instance).
/// - `dt`        : delta time this frame.
///
/// NOTE: The actual allocation / ECS mutation happens inside the host implementation
///       of `spawn_fn`. The plugin only computes when/where to spawn and requests it.
pub fn spawn_enemies(
    spawn_fn: extern "C" fn(*mut HostContext, f32, f32),
    world_ctx: *mut HostContext,
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

        // Plugin never dereferences world_ctx; host will cast it to &mut World internally.
        spawn_fn(world_ctx, rx, ry);
    }
}
