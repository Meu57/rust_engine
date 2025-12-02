// crates/game_plugin/src/lib.rs

use engine_shared::{GameLogic, CTransform, CEnemy, CSprite, Input, CPlayer};
use engine_ecs::{World, Entity};
use glam;

// Simple Game struct
struct MyGame {
    spawn_timer: f32,
}

impl GameLogic for MyGame {
    fn update(&mut self, world_any: &mut dyn std::any::Any, input: &Input, dt: f32) {
        let world = world_any
            .downcast_mut::<World>()
            .expect("Failed to downcast World!");

        // --- INPUT (adjust signs per your raw mapping) ---
        let player_speed = 400.0;
        let mut player_velocity = glam::Vec2::ZERO;
        if input.is_key_pressed(41) { player_velocity.y += 1.0; } // W (your mapping)
        if input.is_key_pressed(37) { player_velocity.y -= 1.0; } // S (your mapping)
        if input.is_key_pressed(19) { player_velocity.x -= 1.0; } // A
        if input.is_key_pressed(22) { player_velocity.x += 1.0; } // D
        if player_velocity.length_squared() > 0.0 {
            player_velocity = player_velocity.normalize() * player_speed * dt;
        }

        // --- 1) READ-ONLY PASS: find the player entity via CPlayer ---
        let mut player_entity_opt: Option<Entity> = None;
        if let Some(players) = world.query::<CPlayer>() {
            for (entity, _player_marker) in players.iter() {
                player_entity_opt = Some(*entity);
                break; // assume single player
            }
        }

        // Fallback: if no CPlayer found, fall back to Entity(0,0)
        let player_entity = player_entity_opt.unwrap_or_else(|| Entity::new(0, 0));

        // --- 2) WRITE PASS: move transforms but only for the player ---
        if let Some(transforms) = world.query_mut::<CTransform>() {
            for (entity, transform) in transforms.iter_mut() {
                if *entity == player_entity {
                    // Only modify the player's transform here
                    transform.pos += player_velocity;
                    transform.pos = transform.pos.clamp(glam::Vec2::ZERO, glam::Vec2::new(1280.0, 720.0));

                    if player_velocity.length_squared() > 0.0 {
                        println!("Player Pos: ({:.1}, {:.1})", transform.pos.x, transform.pos.y);
                    }
                } else {
                    // Keep enemy logic separate (do not log as player)
                }
            }
        }

        // --- 3) SPAWN ENEMIES ---
        self.spawn_timer -= dt;
        if self.spawn_timer <= 0.0 {
            self.spawn_timer = 2.0;
            let enemy = world.spawn();

            let rand_x = (dt * 12345.0).rem_euclid(1280.0);
            let rand_y = (dt * 67890.0).rem_euclid(720.0);

            world.add_component(
                enemy,
                CTransform {
                    pos: glam::Vec2::new(rand_x, rand_y),
                    scale: glam::Vec2::splat(0.8),
                    rotation: 0.0,
                },
            );
            world.add_component(enemy, CEnemy { speed: 100.0 });
            world.add_component(
                enemy,
                CSprite {
                    color: glam::Vec4::new(1.0, 0.0, 0.0, 1.0),
                },
            );
            println!("Spawned Enemy at ({:.1}, {:.1})", rand_x, rand_y);
        }
    }
}

// FFI boundary
#[no_mangle]
pub extern "C" fn _create_game() -> *mut dyn GameLogic {
    let game = Box::new(MyGame {
        spawn_timer: 2.0,
    });
    Box::into_raw(game)
}
