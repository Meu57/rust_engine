// crates/game_plugin/src/lib.rs
mod systems; 

use engine_shared::{GameLogic, InputState, HostInterface, ActionId, ACTION_NOT_FOUND};
use engine_ecs::World;

pub struct MyGame {
    spawn_timer: f32,
    // Store action IDs here
    actions: [ActionId; 4], 
}

impl Default for MyGame {
    fn default() -> Self {
        Self {
            spawn_timer: 2.0,
            actions: [ACTION_NOT_FOUND; 4],
        }
    }
}

impl GameLogic for MyGame {
    fn on_load(&mut self, _world: &mut dyn std::any::Any, host: &HostInterface) {
        // Load IDs once
        self.actions[0] = (host.get_action_id)(b"MoveUp".as_ptr(), 6);
        self.actions[1] = (host.get_action_id)(b"MoveDown".as_ptr(), 8);
        self.actions[2] = (host.get_action_id)(b"MoveLeft".as_ptr(), 8);
        self.actions[3] = (host.get_action_id)(b"MoveRight".as_ptr(), 9);
    }

    fn update(&mut self, world_any: &mut dyn std::any::Any, input: &InputState, dt: f32) {
        let world = world_any.downcast_mut::<World>().expect("Bad world downcast");

        // [FIX] Call the functions by their actual names defined in the files
        systems::player::update_player(world, input, dt, &self.actions);
        systems::enemy::spawn_enemies(world, &mut self.spawn_timer, dt);
    }
}

// ... Keep the `_create_game` FFI function at the bottom ...
#[no_mangle]
pub extern "C" fn _create_game() -> *mut dyn GameLogic {
    let g: Box<dyn GameLogic> = Box::new(MyGame::default());
    Box::into_raw(g)
}