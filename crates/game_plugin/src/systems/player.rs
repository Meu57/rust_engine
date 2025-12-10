// crates/game_plugin/src/systems/player.rs

use engine_ecs::World;
use engine_shared::{InputState, CPlayer, CTransform, ActionId, CWorldBounds};
use glam::Vec2;

pub fn update_player(world: &mut World, input: &InputState, dt: f32, actions: &[ActionId; 4]) {
    let [up, down, left, right] = *actions;

    // 1. Fetch Map Bounds & Debug
    let mut map_size = Vec2::new(1280.0, 720.0); // Default Fallback
    let mut bounds_found = false;

    if let Some(bounds) = world.query::<CWorldBounds>() {
        for (_, b) in bounds.iter() {
            map_size = Vec2::new(b.width, b.height);
            bounds_found = true;
            break; 
        }
    }

    if !bounds_found {
        // This log will spam if bounds are missing, alerting you immediately
        // println!("[WARN] No CWorldBounds found! Using default 1280x720");
    }

    // 2. Calculate Velocity
    let mut direction = Vec2::ZERO;
    if input.is_active(up) { direction.y += 1.0; }
    if input.is_active(down) { direction.y -= 1.0; }
    if input.is_active(left) { direction.x -= 1.0; }
    if input.is_active(right) { direction.x += 1.0; }

    let speed = 600.0;
    let velocity = if direction.length_squared() > 0.0 {
        direction.normalize() * speed * dt
    } else {
        Vec2::ZERO
    };

    // 3. Apply & Clamp with Debug Logging
    let mut player_ids = Vec::new();
    if let Some(players) = world.query::<CPlayer>() {
        for (entity, _) in players.iter() {
            player_ids.push(*entity);
        }
    }

    if let Some(transforms) = world.query_mut::<CTransform>() {
        for entity in player_ids {
            if let Some(transform) = transforms.get_mut(entity) {
                let old_pos = transform.pos;
                transform.pos += velocity;
                
                // Clamp
                transform.pos = transform.pos.clamp(Vec2::ZERO, map_size);

                // [DEBUG] Check if we hit a wall
                if transform.pos != old_pos + velocity {
                    // Only log if we are actually trying to move but got stopped
                    if velocity.length_squared() > 0.0 {
                         // println!("[DEBUG] HIT WALL! Pos: {} | Limit: {}", transform.pos, map_size);
                    }
                }
            }
        }
    }
}