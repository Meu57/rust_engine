// crates/game_plugin/src/systems/player.rs

use engine_ecs::World;
use engine_shared::{InputState, CPlayer, CTransform, ActionId, CWorldBounds};
use glam::Vec2;

pub fn update_player(world: &mut World, input: &InputState, dt: f32, actions: &[ActionId; 4]) {
    let [up, down, left, right] = *actions;

    // 1. Fetch Map Bounds (Dynamic Source of Truth)
    let mut map_size = Vec2::new(1280.0, 720.0); // Safe fallback
    if let Some(bounds) = world.query::<CWorldBounds>() {
        for (_, b) in bounds.iter() {
            map_size = Vec2::new(b.width, b.height);
            break; 
        }
    }

    // 2. Calculate Movement
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

    // 3. Apply Movement & Clamp
    let mut player_ids = Vec::new();
    if let Some(players) = world.query::<CPlayer>() {
        for (entity, _) in players.iter() {
            player_ids.push(*entity);
        }
    }

    if let Some(transforms) = world.query_mut::<CTransform>() {
        for entity in player_ids {
            if let Some(transform) = transforms.get_mut(entity) {
                transform.pos += velocity;
                // [AUDIO FIX] Clamp to DYNAMIC bounds
                transform.pos = transform.pos.clamp(Vec2::ZERO, map_size);
            }
        }
    }
}