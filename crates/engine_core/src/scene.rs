// crates/engine_core/src/scene.rs
use engine_ecs::World;
use engine_shared::{CCamera, CEnemy, CPlayer, CSprite, CTransform, CWorldBounds}; 

pub fn setup_default_world(world: &mut World) {
    world.register_component::<CTransform>();
    world.register_component::<CPlayer>();
    world.register_component::<CEnemy>();
    world.register_component::<CSprite>();
    world.register_component::<CCamera>();
    world.register_component::<CWorldBounds>(); // <--- NEW
}