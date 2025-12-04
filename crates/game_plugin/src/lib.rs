// crates/game_plugin/src/lib.rs

mod systems;

use std::ffi::c_void;
use engine_shared::{GameLogic, InputState, HostInterface, ActionId, ACTION_NOT_FOUND, ENGINE_API_VERSION};
use engine_ecs::World;

/// Game plugin instance
pub struct MyGame {
    spawn_timer: f32,
    actions: [ActionId; 4],
    // Optional host-provided spawn function pointer (set in on_load)
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

impl GameLogic for MyGame {
    fn on_load(&mut self, _world: &mut dyn std::any::Any, host: &HostInterface) {
        // Resolve action IDs from host
        self.actions[0] = (host.get_action_id)(b"MoveUp".as_ptr(), 6);
        self.actions[1] = (host.get_action_id)(b"MoveDown".as_ptr(), 8);
        self.actions[2] = (host.get_action_id)(b"MoveLeft".as_ptr(), 8);
        self.actions[3] = (host.get_action_id)(b"MoveRight".as_ptr(), 9);

        // Save host-provided spawn function (if available)
        // Note: HostInterface.spawn_enemy must be declared as `extern "C" fn(*mut c_void, f32, f32)`
        // in engine_shared::HostInterface. If HostInterface.spawn_enemy is optional (Option<...>),
        // adjust this assignment accordingly.
        self.spawn_fn = Some(host.spawn_enemy);
    }

    fn update(&mut self, world_any: &mut dyn std::any::Any, input: &InputState, dt: f32) {
        // Downcast to the concrete World only to read/query local state in the plugin.
        // The plugin must NOT mutate the host world directly.
        let world = world_any.downcast_mut::<World>().expect("Bad world downcast");

        // Player update uses the action IDs resolved earlier
        systems::player::update_player(world, input, dt, &self.actions);

        // If host spawn function is present, call it via opaque pointer
        if let Some(spawn_fn) = self.spawn_fn {
            // Cast World to void pointer (opaque to plugin, host will cast back)
            let world_ptr = world as *mut World as *mut c_void;

            // Delegate enemy spawning to systems::enemy, which will call the host function.
            // systems::enemy::spawn_enemies should accept:
            //   (spawn_fn: extern "C" fn(*mut c_void, f32, f32), world_ptr: *mut c_void, spawn_timer: &mut f32, dt: f32)
            systems::enemy::spawn_enemies(spawn_fn, world_ptr, &mut self.spawn_timer, dt);
        } else {
            // Fallback: if the host didn't provide spawn function, you could avoid spawning
            // or use a local fallback (not recommended). For now, do nothing.
        }
    }
}

//
// FFI exports
//

#[no_mangle]
pub extern "C" fn _create_game() -> *mut dyn GameLogic {
    let g: Box<dyn GameLogic> = Box::new(MyGame::default());
    Box::into_raw(g)
}

#[no_mangle]
pub extern "C" fn get_api_version() -> u32 {
    ENGINE_API_VERSION
}
