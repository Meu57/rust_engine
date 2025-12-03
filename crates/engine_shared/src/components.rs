// crates/engine_shared/src/components.rs
use glam::{Vec2, Vec4};

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CTransform {
    pub pos: Vec2,
    pub scale: Vec2,
    pub rotation: f32,
}

impl Default for CTransform {
    fn default() -> Self {
        Self { pos: Vec2::ZERO, scale: Vec2::ONE, rotation: 0.0 }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CSprite {
    pub color: Vec4,
}

impl Default for CSprite {
    fn default() -> Self { Self { color: Vec4::ONE } }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CPlayer;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CEnemy {
    pub speed: f32,
}