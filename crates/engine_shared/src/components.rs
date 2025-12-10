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

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CCamera {
    pub zoom: f32,
    pub smoothness: f32, 
}

impl Default for CCamera {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            smoothness: 5.0,
        }
    }
}

// [AUDIO FIX] "Single Source of Truth" Component
// This solves the "Invisible Prison" by ensuring Player & Camera share exact bounds.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CWorldBounds {
    pub width: f32,
    pub height: f32,
}

impl Default for CWorldBounds {
    fn default() -> Self {
        Self { width: 2000.0, height: 2000.0 }
    }
}