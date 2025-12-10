// crates/game_plugin/src/systems/player.rs

use engine_ecs::World;
use engine_shared::{InputState, CPlayer, CTransform, ActionId, CWorldBounds};
use glam::Vec2;

pub fn update_player(world: &mut World, input: &InputState, dt: f32, actions: &[ActionId; 4]) {
    let [up, down, left, right] = *actions;

    // 1. Fetch Map Bounds (CENTERED LOGIC)
    // We convert the 2000.0 size into a range of -1000.0 to +1000.0
    // This removes the "Plus Sign" wall at 0,0.
    let mut min_bound = Vec2::new(-1000.0, -1000.0);
    let mut max_bound = Vec2::new(1000.0, 1000.0);
    
    if let Some(bounds) = world.query::<CWorldBounds>() {
        for (_, b) in bounds.iter() {
            let half_w = b.width / 2.0;
            let half_h = b.height / 2.0;
            min_bound = Vec2::new(-half_w, -half_h);
            max_bound = Vec2::new(half_w, half_h);
            break; 
        }
    }

    // 2. Identify Inputs
    let mut pressed_buttons = Vec::new();
    let mut direction = Vec2::ZERO;

    if input.is_active(up) { 
        direction.y += 1.0; 
        pressed_buttons.push("UP");
    }
    if input.is_active(down) { 
        direction.y -= 1.0; 
        pressed_buttons.push("DOWN");
    }
    if input.is_active(left) { 
        direction.x -= 1.0; 
        pressed_buttons.push("LEFT");
    }
    if input.is_active(right) { 
        direction.x += 1.0; 
        pressed_buttons.push("RIGHT");
    }

    let speed = 600.0;
    // This is the move we INTEND to make
    let expected_velocity = if direction.length_squared() > 0.0 {
        direction.normalize() * speed * dt
    } else {
        Vec2::ZERO
    };

    // 3. Apply Movement & Debug
    let mut player_ids = Vec::new();
    if let Some(players) = world.query::<CPlayer>() {
        for (entity, _) in players.iter() {
            player_ids.push(*entity);
        }
    }

    if let Some(transforms) = world.query_mut::<CTransform>() {
        for entity in player_ids {
            if let Some(transform) = transforms.get_mut(entity) {
                let start_pos = transform.pos;
                
                // Try to move
                let target_pos = start_pos + expected_velocity;
                
                // [FIX] Clamp to Centered Bounds (allows crossing 0,0)
                let clamped_pos = target_pos.clamp(min_bound, max_bound);
                
                transform.pos = clamped_pos;

                // --- SILENT DEBUG LOGIC ---
                // Only run if we are actually pressing buttons
                if !pressed_buttons.is_empty() {
                    let actual_dist = transform.pos.distance(start_pos);
                    let expected_dist = expected_velocity.length();

                    // If we wanted to move (expected > 0) but moved less than 0.001 units
                    if expected_dist > 0.001 && actual_dist < 0.001 {
                        println!(
                            "[STUCK] Buttons: {:?} | Pos: {} | Wall Limit: {} to {}", 
                            pressed_buttons, start_pos, min_bound, max_bound
                        );
                    }
                    // Else: We are moving fine. Do not print anything.
                }
            }
        }
    }
}