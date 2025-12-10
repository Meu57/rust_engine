//crates\game_plugin\src\systems\player.rs

use engine_ecs::World;
use engine_shared::{InputState, CPlayer, CTransform, ActionId};
use glam::Vec2;

pub fn update_player(world: &mut World, input: &InputState, dt: f32, actions: &[ActionId; 4]) {
    let [up, down, left, right] = *actions;

    // Build directional intent
    let mut direction = Vec2::ZERO;
    if input.is_active(up) {
        direction.y += 1.0;
    }
    if input.is_active(down) {
        direction.y -= 1.0;
    }
    if input.is_active(left) {
        direction.x -= 1.0;
    }
    if input.is_active(right) {
        // [FIX] 1000.0 -> 1.0 to avoid insane vectors
        direction.x += 1.0;
    }

    // Movement magnitude (tweakable)
    let speed = 600.0;

    // Normalize direction, then apply magnitude and delta-time
    let velocity = if direction.length_squared() > 0.0 {
        direction.normalize() * speed * dt
    } else {
        Vec2::ZERO
    };

    // --- OPTIMIZED QUERY ---

    // 1) Collect player entity IDs (usually a small set)
    let mut player_ids = Vec::new();
    if let Some(players) = world.query::<CPlayer>() {
        for (entity, _) in players.iter() {
            player_ids.push(*entity);
        }
    }

    // 2) For each player, do an O(1) lookup on the transform storage
    if let Some(transforms) = world.query_mut::<CTransform>() {
        for entity in player_ids {
            if let Some(transform) = transforms.get_mut(entity) {
                transform.pos += velocity;
                transform.pos = transform
                    .pos
                    .clamp(Vec2::ZERO, Vec2::new(1280.0, 720.0));
            }
        }
    }
}
