use engine_ecs::{World, Entity};
use engine_shared::{InputState, CPlayer, CTransform, ActionId};
use glam::Vec2;

pub fn update_player(world: &mut World, input: &InputState, dt: f32, actions: &[ActionId; 4]) {
    let [up, down, left, right] = *actions;
    let mut velocity = Vec2::ZERO;
    let speed = 400.0;

    if input.is_active(up) { velocity.y += 1.0; }
    if input.is_active(down) { velocity.y -= 1.0; }
    if input.is_active(left) { velocity.x -= 1.0; }
    if input.is_active(right) { velocity.x += 1.0; }

    if velocity.length_squared() > 0.0 {
        velocity = velocity.normalize() * speed * dt;
    }

    
    
    // 1. Collect all entities that are tagged as 'Player'
    // We do this in a separate block/scope to ensure we are done borrowing 'world' 
    // before we try to borrow it mutably in the next step.
    let mut player_ids = Vec::new();
    if let Some(players) = world.query::<CPlayer>() {
        for (entity, _) in players.iter() {
            player_ids.push(*entity);
        }
    }

    // 2. Iterate over Transforms and ONLY move the entities we identified as Players
    if let Some(mut query) = world.query_mut::<CTransform>() {
        for (entity, transform) in query.iter_mut() {
            // Check if this specific entity is in our list of players
            if player_ids.contains(entity) {
                transform.pos += velocity;
                transform.pos = transform.pos.clamp(Vec2::ZERO, Vec2::new(1280.0, 720.0));
            }
        }
    }
    
}