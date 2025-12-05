// crates/game_plugin/src/lib.rs

mod systems;

use std::ffi::c_void;

use engine_shared::{
    PluginApi, HostInterface, InputState, GameLogic, ActionId, ACTION_NOT_FOUND, ENGINE_API_VERSION,
};
use engine_ecs::World;

/// Concrete plugin instance
pub struct MyGame {
    spawn_timer: f32,
    actions: [ActionId; 4],
    spawn_fn: Option<extern "C" fn(*mut c_void, f32, f32)>,
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
    // Accept &mut World directly (host guarantees the pointer is a World)
    fn do_on_load(&mut self, world: &mut World, host: &HostInterface) {
        // Resolve action ids via host (slice-style names)
        self.actions[0] = (host.get_action_id)(b"MoveUp".as_ptr(), b"MoveUp".len());
        self.actions[1] = (host.get_action_id)(b"MoveDown".as_ptr(), b"MoveDown".len());
        self.actions[2] = (host.get_action_id)(b"MoveLeft".as_ptr(), b"MoveLeft".len());
        self.actions[3] = (host.get_action_id)(b"MoveRight".as_ptr(), b"MoveRight".len());

        // Capture host-provided spawn function (if present)
        // Note: if HostInterface.spawn_enemy is Option<...>, handle accordingly.
        self.spawn_fn = Some(host.spawn_enemy);
    }

    // Now takes &mut World instead of dyn Any
    fn do_update(&mut self, world: &mut World, input: &InputState, dt: f32) {
        // Update player logic (reads input, writes via safe queries)
        systems::player::update_player(world, input, dt, &self.actions);

        // Spawn via host callback (allocation happens in host)
        if let Some(spawn_fn) = self.spawn_fn {
            let world_ptr = world as *mut World as *mut c_void;
            systems::enemy::spawn_enemies(spawn_fn, world_ptr, &mut self.spawn_timer, dt);
        }
    }
}

// -------------------------
// FFI Shim Functions
// -------------------------
extern "C" fn shim_on_load(state: *mut c_void, world: *mut c_void, host: &HostInterface) {
    if state.is_null() || world.is_null() { return; }
    unsafe {
        let game: &mut MyGame = &mut *(state as *mut MyGame);

        // Cast void* -> *mut World, then deref to &mut World
        let world_ptr = world as *mut World;
        let world_ref = &mut *world_ptr;

        game.do_on_load(world_ref, host);
    }
}

extern "C" fn shim_update(state: *mut c_void, world: *mut c_void, input: &InputState, dt: f32) {
    if state.is_null() || world.is_null() { return; }
    unsafe {
        let game: &mut MyGame = &mut *(state as *mut MyGame);

        // Cast void* -> *mut World, then deref to &mut World
        let world_ptr = world as *mut World;
        let world_ref = &mut *world_ptr;

        game.do_update(world_ref, input, dt);
    }
}

extern "C" fn shim_on_unload(_state: *mut c_void, _world: *mut c_void) {
    // Optional; left empty for now.
}

extern "C" fn shim_drop(state: *mut c_void) {
    if state.is_null() { return; }
    unsafe {
        // Free plugin-owned heap memory safely inside the plugin.
        let _boxed: Box<MyGame> = Box::from_raw(state as *mut MyGame);
        // dropped here
    }
}

// -------------------------
// Exports
// -------------------------
#[no_mangle]
pub extern "C" fn _create_game() -> PluginApi {
    let boxed = Box::new(MyGame::default());
    let state_ptr = Box::into_raw(boxed) as *mut c_void;

    PluginApi {
        state: state_ptr,
        on_load: shim_on_load,
        update: shim_update,
        on_unload: shim_on_unload,
        drop: shim_drop,
    }
}

#[no_mangle]
pub extern "C" fn get_api_version() -> u32 {
    ENGINE_API_VERSION
}
