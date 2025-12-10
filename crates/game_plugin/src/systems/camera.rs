// crates/game_plugin/src/systems/camera.rs

use engine_ecs::World;
use engine_shared::{CCamera, CPlayer, CTransform, CWorldBounds};
use glam::Vec2;

const VIEWPORT_W: f32 = 1280.0;
const VIEWPORT_H: f32 = 720.0;

pub fn update_camera(world: &mut World, dt: f32) {
    // 1. Fetch Map Bounds
    let mut map_w = 2000.0;
    let mut map_h = 2000.0;
    if let Some(bounds) = world.query::<CWorldBounds>() {
        for (_, b) in bounds.iter() {
            map_w = b.width;
            map_h = b.height;
            break;
        }
    }

    // 2. Find Target
    let mut target_pos = Option::<Vec2>::None;
    if let Some(players) = world.query::<CPlayer>() {
        for (entity, _) in players.iter() {
            if let Some(transform) = world.get_component::<CTransform>(*entity) {
                target_pos = Some(transform.pos);
                break; 
            }
        }
    }
    let Some(target) = target_pos else { return };

    // 3. Collect & Update
    let mut cameras_to_update = Vec::new();
    if let Some(cameras) = world.query::<CCamera>() {
        for (entity, cam_settings) in cameras.iter() {
            cameras_to_update.push((*entity, *cam_settings));
        }
    }

    if let Some(transforms) = world.query_mut::<CTransform>() {
        for (entity, cam_settings) in cameras_to_update {
            if let Some(transform) = transforms.get_mut(entity) {
                
                // Smooth Follow
                let decay = (-cam_settings.smoothness * dt).exp();
                let t = 1.0 - decay;
                transform.pos = transform.pos.lerp(target, t);

                // Bounds Consistency (Using Dynamic Bounds)
                let half_w = VIEWPORT_W / 2.0;
                let half_h = VIEWPORT_H / 2.0;

                let min_x = half_w;
                let max_x = map_w - half_w;
                let min_y = half_h;
                let max_y = map_h - half_h;

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