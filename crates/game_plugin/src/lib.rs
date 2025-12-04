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
    // Logic split into "do_" functions to keep FFI shims clean
    fn do_on_load(&mut self, _world_any: &mut dyn std::any::Any, host: &HostInterface) {
        // Resolve action ids via host
        self.actions[0] = (host.get_action_id)(b"MoveUp".as_ptr(), 6);
        self.actions[1] = (host.get_action_id)(b"MoveDown".as_ptr(), 8);
        self.actions[2] = (host.get_action_id)(b"MoveLeft".as_ptr(), 8);
        self.actions[3] = (host.get_action_id)(b"MoveRight".as_ptr(), 9);

        // Capture the host-provided spawn function
        self.spawn_fn = Some(host.spawn_enemy);
    }

    fn do_update(&mut self, world_any: &mut dyn std::any::Any, input: &InputState, dt: f32) {
        // DOWNCAST ONLY FOR READ/QUERY (plugin should not perform host allocations directly)
        let world = match world_any.downcast_mut::<World>() {
            Some(w) => w,
            None => {
                eprintln!("Plugin Error: Failed to downcast World.");
                return; 
            }, 
        };

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
        let world_any: &mut dyn std::any::Any = &mut *(world as *mut dyn std::any::Any);
        game.do_on_load(world_any, host);
    }
}

extern "C" fn shim_update(state: *mut c_void, world: *mut c_void, input: &InputState, dt: f32) {
    if state.is_null() || world.is_null() { return; }
    unsafe {
        let game: &mut MyGame = &mut *(state as *mut MyGame);
        let world_any: &mut dyn std::any::Any = &mut *(world as *mut dyn std::any::Any);
        game.do_update(world_any, input, dt);
    }
}

extern "C" fn shim_on_unload(_state: *mut c_void, _world: *mut c_void) {
    // Optional; left empty for now.
}

extern "C" fn shim_drop(state: *mut c_void) {
    if state.is_null() { return; }
    unsafe {
        // Box::from_raw to free plugin-owned heap memory safely inside the plugin.
        let _boxed: Box<MyGame> = Box::from_raw(state as *mut MyGame);
        // _boxed is dropped here, freeing memory
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