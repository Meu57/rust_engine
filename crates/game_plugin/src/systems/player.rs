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

    if let Some(mut query) = world.query_mut::<CTransform>() {
        // Simple query for player (naive iteration for demo)
        // In a real engine, you'd join CPlayer and CTransform
        for (_, transform) in query.iter_mut() {
             // Add logic to check if this entity also has CPlayer (omitted for brevity)
             transform.pos += velocity;
             transform.pos = transform.pos.clamp(Vec2::ZERO, Vec2::new(1280.0, 720.0));
        }
    }
}