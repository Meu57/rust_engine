// crates/game_plugin/src/systems/camera.rs

use engine_ecs::World;
use engine_shared::{CCamera, CPlayer, CTransform};
use glam::Vec2;

// Map Settings (You can adjust these)
const MAP_WIDTH: f32 = 2000.0;
const MAP_HEIGHT: f32 = 2000.0;
const VIEWPORT_W: f32 = 1280.0;
const VIEWPORT_H: f32 = 720.0;

pub fn update_camera(world: &mut World, dt: f32) {
    // 1. Identify the Target (The Player)
    let mut target_pos = Option::<Vec2>::None;
    
    if let Some(players) = world.query::<CPlayer>() {
        for (entity, _) in players.iter() {
            if let Some(transform) = world.get_component::<CTransform>(*entity) {
                target_pos = Some(transform.pos);
                break; // Focus on the first player found
            }
        }
    }

    // If no player exists, do nothing
    let Some(target) = target_pos else { return };

    // 2. Collect Camera Data (READ PHASE)
    // We collect the Entity ID and Settings into a temporary vector.
    // This allows the borrow on 'world' to end immediately after this block.
    let mut cameras_to_update = Vec::new();
    
    if let Some(cameras) = world.query::<CCamera>() {
        for (entity, cam_settings) in cameras.iter() {
            cameras_to_update.push((*entity, *cam_settings));
        }
    }

    // 3. Update Transforms (WRITE PHASE)
    // Now we can safely borrow 'world' mutably because the previous borrow is dropped.
    if let Some(transforms) = world.query_mut::<CTransform>() {
        for (entity, cam_settings) in cameras_to_update {
            if let Some(transform) = transforms.get_mut(entity) {
                
                // --- A. Smooth Follow ---
                let decay = (-cam_settings.smoothness * dt).exp();
                let t = 1.0 - decay;
                transform.pos = transform.pos.lerp(target, t);

                // --- B. Bounds Consistency ---
                let half_w = VIEWPORT_W / 2.0;
                let half_h = VIEWPORT_H / 2.0;

                let min_x = half_w;
                let max_x = MAP_WIDTH - half_w;
                let min_y = half_h;
                let max_y = MAP_HEIGHT - half_h;

                if max_x > min_x {
                    transform.pos.x = transform.pos.x.clamp(min_x, max_x);
                }
                if max_y > min_y {
                    transform.pos.y = transform.pos.y.clamp(min_y, max_y);
                }
            }
        }
    }
}