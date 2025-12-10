// crates/game_plugin/src/systems/camera.rs

use engine_ecs::World;
use engine_shared::{CCamera, CPlayer, CTransform, CWorldBounds};
use glam::Vec2;

const VIEWPORT_W: f32 = 1280.0;
const VIEWPORT_H: f32 = 720.0;

// [NEW] DEADZONE SETTINGS
// The camera will NOT move as long as the player is within this box.
// This cures the "Treadmill Effect" because you can see yourself move.
const DEADZONE_W: f32 = 100.0; // Player can move 100px left/right before camera follows
const DEADZONE_H: f32 = 80.0;  // Player can move 80px up/down before camera follows

pub fn update_camera(world: &mut World, dt: f32) {
    // 1. Fetch Map Bounds (Centered)
    let mut half_map_w = 1000.0;
    let mut half_map_h = 1000.0;
    
    if let Some(bounds) = world.query::<CWorldBounds>() {
        for (_, b) in bounds.iter() {
            half_map_w = b.width / 2.0;
            half_map_h = b.height / 2.0;
            break;
        }
    }

    // 2. Find Target (Player)
    let mut target_pos = Option::<Vec2>::None;
    if let Some(players) = world.query::<CPlayer>() {
        for (entity, _) in players.iter() {
            if let Some(transform) = world.get_component::<CTransform>(*entity) {
                target_pos = Some(transform.pos);
                break; 
            }
        }
    }
    let Some(player_pos) = target_pos else { return };

    // 3. Update Camera with DEADZONE
    let mut cameras_to_update = Vec::new();
    if let Some(cameras) = world.query::<CCamera>() {
        for (entity, cam_settings) in cameras.iter() {
            cameras_to_update.push((*entity, *cam_settings));
        }
    }

    if let Some(transforms) = world.query_mut::<CTransform>() {
        for (entity, cam_settings) in cameras_to_update {
            if let Some(cam_transform) = transforms.get_mut(entity) {
                
                // --- DEADZONE LOGIC ---
                // Calculate how far the player is from the Camera Center
                let delta = player_pos - cam_transform.pos;

                // Calculate where the camera "should" be to keep player in bounds
                let mut desired_x = cam_transform.pos.x;
                let mut desired_y = cam_transform.pos.y;

                // X-Axis Deadzone
                if delta.x > DEADZONE_W {
                    // Player pushed the Right edge -> Move Camera Right
                    desired_x = player_pos.x - DEADZONE_W;
                } else if delta.x < -DEADZONE_W {
                    // Player pushed the Left edge -> Move Camera Left
                    desired_x = player_pos.x + DEADZONE_W;
                }

                // Y-Axis Deadzone
                if delta.y > DEADZONE_H {
                    // Player pushed Top edge
                    desired_y = player_pos.y - DEADZONE_H;
                } else if delta.y < -DEADZONE_H {
                    // Player pushed Bottom edge
                    desired_y = player_pos.y + DEADZONE_H;
                }

                let target_cam_pos = Vec2::new(desired_x, desired_y);

                // Smoothly slide to the new "Edge" position
                let decay = (-cam_settings.smoothness * dt).exp();
                let t = 1.0 - decay;
                cam_transform.pos = cam_transform.pos.lerp(target_cam_pos, t);

                // --- MAP BOUNDS CLAMPING (Same as before) ---
                let half_view_w = VIEWPORT_W / 2.0;
                let half_view_h = VIEWPORT_H / 2.0;

                let max_x = half_map_w - half_view_w;
                let min_x = -max_x;
                
                let max_y = half_map_h - half_view_h;
                let min_y = -max_y;

                if half_map_w > half_view_w {
                    cam_transform.pos.x = cam_transform.pos.x.clamp(min_x, max_x);
                }
                if half_map_h > half_view_h {
                    cam_transform.pos.y = cam_transform.pos.y.clamp(min_y, max_y);
                }
            }
        }
    }
}