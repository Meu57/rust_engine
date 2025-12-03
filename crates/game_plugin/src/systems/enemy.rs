use engine_ecs::World;
use engine_shared::{CTransform, CEnemy, CSprite};
use glam::{Vec2, Vec4};

pub fn spawn_enemies(world: &mut World, timer: &mut f32, dt: f32) {
    *timer -= dt;
    if *timer <= 0.0 {
        *timer = 2.0;
        let enemy = world.spawn();
        let rx = (dt * 12345.0).rem_euclid(1280.0); // Pseudo-random placeholder
        let ry = (dt * 67890.0).rem_euclid(720.0);
        
        world.add_component(enemy, CTransform { 
            pos: Vec2::new(rx, ry), 
            scale: Vec2::splat(0.8), 
            rotation: 0.0 
        });
        world.add_component(enemy, CEnemy { speed: 100.0 });
        world.add_component(enemy, CSprite { color: Vec4::new(1.0, 0.0, 0.0, 1.0) });
    }
}