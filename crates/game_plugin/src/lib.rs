// crates/game_plugin/src/lib.rs

mod systems;

use std::ffi::c_void;

use engine_ecs::World;
use engine_shared::{
    ActionId,
    HostContext,
    HostInterface,
    InputState,
    PluginApi,
    ACTION_NOT_FOUND,
    ENGINE_API_VERSION,
    calculate_layout_hash,
};

/// Concrete plugin instance
pub struct MyGame {
    spawn_timer: f32,
    actions: [ActionId; 4],
    // HostContext is an opaque handle to the host's world/context.
    spawn_fn: Option<extern "C" fn(*mut HostContext, f32, f32)>,
}

impl Default for MyGame {
    fn default() -> Self {
        Self {
            spawn_timer: 2.0,
            actions: [ACTION_NOT_FOUND; 4],
            spawn_fn: None,
        }
    }
}

impl MyGame {
    /// Called when the plugin is loaded.
    /// Host guarantees `world` really is its ECS World.
    fn do_on_load(&mut self, _world: &mut World, host: &HostInterface) {
        // Resolve action ids via host (slice-style names)
        self.actions[0] = (host.get_action_id)(b"MoveUp".as_ptr(), b"MoveUp".len());
        self.actions[1] = (host.get_action_id)(b"MoveDown".as_ptr(), b"MoveDown".len());
        self.actions[2] = (host.get_action_id)(b"MoveLeft".as_ptr(), b"MoveLeft".len());
        self.actions[3] = (host.get_action_id)(b"MoveRight".as_ptr(), b"MoveRight".len());

        // Capture host-provided spawn function
        self.spawn_fn = Some(host.spawn_enemy);
    }

    /// Per-frame update.
    fn do_update(&mut self, world: &mut World, input: &InputState, dt: f32) {
        // Player logic (reads input, writes via safe ECS)
        systems::player::update_player(world, input, dt, &self.actions);

        // Enemy spawning via host callback (allocation happens in host)
        if let Some(spawn_fn) = self.spawn_fn {
            // SAFETY: We cast &mut World to *mut HostContext.
            // This is safe ONLY because we know the Host interprets HostContext as World.
            let world_ptr = world as *mut World as *mut HostContext;
            systems::enemy::spawn_enemies(spawn_fn, world_ptr, &mut self.spawn_timer, dt);
        }
    }
}

// --- FFI SHIMS ---

extern "C" fn shim_on_load(
    state: *mut c_void,
    ctx: *mut HostContext,
    host: &HostInterface,
) {
    if state.is_null() || ctx.is_null() {
        return;
    }

    unsafe {
        let game: &mut MyGame = &mut *(state as *mut MyGame);
        let world: &mut World = &mut *(ctx as *mut World); // Cast opaque handle back to World
        game.do_on_load(world, host);
    }
}

extern "C" fn shim_update(
    state: *mut c_void,
    ctx: *mut HostContext,
    input: &InputState,
    dt: f32,
) {
    if state.is_null() || ctx.is_null() {
        return;
    }

    unsafe {
        let game: &mut MyGame = &mut *(state as *mut MyGame);
        let world: &mut World = &mut *(ctx as *mut World);
        game.do_update(world, input, dt);
    }
}

extern "C" fn shim_on_unload(_state: *mut c_void, _ctx: *mut HostContext) {
    // Optional; no special unload behavior yet.
}

extern "C" fn shim_drop(state: *mut c_void) {
    if state.is_null() {
        return;
    }

    unsafe {
        // Free plugin-owned heap memory safely inside the plugin.
        let _boxed: Box<MyGame> = Box::from_raw(state as *mut MyGame);
        // dropped here
    }
}

extern "C" fn shim_get_hash() -> u64 {
    // Return the hash of the structs we rely on (currently just InputState).
    calculate_layout_hash("InputState")
}

// --- EXPORTS ---

#[no_mangle]
pub extern "C" fn _create_game() -> PluginApi {
    let state = Box::into_raw(Box::new(MyGame::default())) as *mut c_void;

    PluginApi {
        state,
        on_load: shim_on_load,
        update: shim_update,
        on_unload: shim_on_unload,
        drop: shim_drop,
        get_layout_hash: shim_get_hash,
    }
}

#[no_mangle]
pub extern "C" fn get_api_version() -> u32 {
    ENGINE_API_VERSION
}
