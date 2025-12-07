// crates/engine_core/src/host.rs
use crate::input;
use engine_ecs::World;
use engine_shared::{CEnemy, CSprite, CTransform, HostContext, HostInterface};
use glam::Vec2;

/// The implementation of the spawn function provided to the plugin.
extern "C" fn host_spawn_enemy(ctx: *mut HostContext, x: f32, y: f32) {
    if ctx.is_null() {
        eprintln!("host_spawn_enemy called with null HostContext");
        return;
    }

    unsafe {
        // Cast HostContext back to World.
        let world = &mut *(ctx as *mut World);

        let enemy = world.spawn();
        world.add_component(
            enemy,
            CTransform {
                pos: Vec2::new(x, y),
                scale: Vec2::splat(0.8),
                rotation: 0.0,
            },
        );
        world.add_component(enemy, CEnemy { speed: 100.0 });
        world.add_component(
            enemy,
            CSprite {
                color: glam::Vec4::new(1.0, 0.0, 0.0, 1.0),
            },
        );
    }
}

/// Helper to construct the interface struct
pub fn create_interface() -> HostInterface {
    HostInterface {
        get_action_id: input::host_get_action_id,
        log: None,
        spawn_enemy: host_spawn_enemy,
    }
}