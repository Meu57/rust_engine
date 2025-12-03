// crates/game_plugin/src/lib.rs
#![allow(dead_code)]

use engine_shared::{
    GameLogic, InputState, CTransform, CSprite, CEnemy, CPlayer, HostInterface,
    ActionId, ACTION_NOT_FOUND,
};
use engine_ecs::{World, Entity};
use glam::Vec2;

/// The plugin's game instance
pub struct MyGame {
    spawn_timer: f32,
    // cached action ids resolved during on_load()
    action_move_up: ActionId,
    action_move_down: ActionId,
    action_move_left: ActionId,
    action_move_right: ActionId,
}

impl Default for MyGame {
    fn default() -> Self {
        Self {
            spawn_timer: 2.0,
            action_move_up: ACTION_NOT_FOUND,
            action_move_down: ACTION_NOT_FOUND,
            action_move_left: ACTION_NOT_FOUND,
            action_move_right: ACTION_NOT_FOUND,
        }
    }
}

impl GameLogic for MyGame {
    fn on_load(&mut self, _world: &mut dyn std::any::Any, host: &HostInterface) {
        // ask host for ids; pass bytes + len
        self.action_move_up = (host.get_action_id)(b"MoveUp".as_ptr(), 6);
        self.action_move_down = (host.get_action_id)(b"MoveDown".as_ptr(), 8);
        self.action_move_left = (host.get_action_id)(b"MoveLeft".as_ptr(), 8);
        self.action_move_right = (host.get_action_id)(b"MoveRight".as_ptr(), 9);

        // optional logging
        if let Some(log_fn) = host.log {
            use std::ffi::CString;
            let msg = CString::new("Plugin: on_load complete").unwrap();
            log_fn(msg.as_ptr());
        }
    }

    fn update(&mut self, world_any: &mut dyn std::any::Any, input: &InputState, dt: f32) {
        let world = world_any.downcast_mut::<World>().expect("Bad world downcast");

        let mut player_velocity = Vec2::ZERO;
        let speed = 400.0;

        if input.is_active(self.action_move_up) { player_velocity.y += 1.0; }
        if input.is_active(self.action_move_down) { player_velocity.y -= 1.0; }
        if input.is_active(self.action_move_left) { player_velocity.x -= 1.0; }
        if input.is_active(self.action_move_right) { player_velocity.x += 1.0; }

        if player_velocity.length_squared() > 0.0 {
            player_velocity = player_velocity.normalize() * speed * dt;
        }

        // find player entity
        let mut player_entity_opt: Option<Entity> = None;
        if let Some(query) = world.query::<CPlayer>() {
            for (ent, _m) in query.iter() {
                player_entity_opt = Some(*ent);
                break;
            }
        }
        let player_entity = player_entity_opt.unwrap_or_else(|| Entity::new(0,0));

        // move player transform
        if let Some(mut query) = world.query_mut::<CTransform>() {
            for (ent, transform) in query.iter_mut() {
                if *ent == player_entity {
                    transform.pos += player_velocity;
                    transform.pos = transform.pos.clamp(Vec2::ZERO, Vec2::new(1280.0,720.0));
                }
            }
        }

        // spawn enemies timer
        self.spawn_timer -= dt;
        if self.spawn_timer <= 0.0 {
            self.spawn_timer = 2.0;
            let enemy = world.spawn();
            let rx = (dt * 12345.0).rem_euclid(1280.0);
            let ry = (dt * 67890.0).rem_euclid(720.0);
            world.add_component(enemy, CTransform { pos: Vec2::new(rx, ry), scale: Vec2::splat(0.8), rotation: 0.0 });
            world.add_component(enemy, CEnemy { speed: 100.0 });
            world.add_component(enemy, CSprite { color: glam::Vec4::new(1.0, 0.0, 0.0, 1.0) });
        }
    }
}

/// FFI factory exported for host to create plugin instance.
/// Must be `extern "C"` and `#[no_mangle]`. Host expects pointer to boxed trait object.
#[no_mangle]
pub extern "C" fn _create_game() -> *mut dyn GameLogic {
    let g: Box<dyn GameLogic> = Box::new(MyGame::default());
    Box::into_raw(g)
}
