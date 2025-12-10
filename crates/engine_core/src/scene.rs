// crates/engine_core/src/scene.rs
use engine_ecs::World;
use engine_shared::{CCamera, CEnemy, CPlayer, CSprite, CTransform}; 
use glam::Vec2;

pub fn setup_default_world(world: &mut World) {
    // 1. Register Components
    world.register_component::<CTransform>();
    world.register_component::<CPlayer>();
    world.register_component::<CEnemy>();
    world.register_component::<CSprite>();
    world.register_component::<CCamera>(); // <--- NEW

    // 2. Spawn Player
    let player = world.spawn();
    world.add_component(
        player,
        CTransform {
            pos: Vec2::new(100.0, 100.0),
            ..Default::default()
        },
    );
    world.add_component(player, CPlayer);
    world.add_component(player, CSprite::default());

    // 3. Spawn Camera (NEW)
    let camera = world.spawn();
    world.add_component(camera, CTransform::default());
    world.add_component(camera, CCamera {
        zoom: 1.0,
        smoothness: 5.0, // Tweak this for camera feel
    });
}