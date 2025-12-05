// crates/engine_core/src/renderer/extractor.rs
use engine_ecs::World;
use engine_shared::{CTransform, CSprite};
use glam::{Mat4, Vec3};
use crate::renderer::instance::InstanceRaw;

/// Extract InstanceRaw data from the ECS world (simple, single-pass)
pub fn extract_instances(world: &World) -> Vec<InstanceRaw> {
    let mut instances = Vec::new();
    if let (Some(transforms), Some(sprites)) = (world.query::<CTransform>(), world.query::<CSprite>()) {
        for (entity, transform) in transforms.iter() {
            if let Some(sprite) = sprites.get(*entity) {
                let model = Mat4::from_scale_rotation_translation(
                    Vec3::new(transform.scale.x * 50.0, transform.scale.y * 50.0, 1.0),
                    glam::Quat::from_rotation_z(transform.rotation),
                    Vec3::new(transform.pos.x, transform.pos.y, 0.0),
                );
                instances.push(InstanceRaw {
                    model: model.to_cols_array_2d(),
                    color: sprite.color.to_array(),
                });
            }
        }
    }
    instances
}
