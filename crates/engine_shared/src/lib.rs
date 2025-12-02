// crates/engine_shared/src/lib.rs

use glam::{Vec2, Vec4};
use std::collections::HashSet;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CTransform {
    pub pos: Vec2,
    pub scale: Vec2,
    pub rotation: f32,
}

impl Default for CTransform {
    fn default() -> Self {
        Self {
            pos: Vec2::ZERO,
            scale: Vec2::ONE,
            rotation: 0.0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CSprite {
    pub color: Vec4,
}

impl Default for CSprite {
    fn default() -> Self {
        Self {
            color: Vec4::new(1.0, 1.0, 1.0, 1.0),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CPlayer;

// Enemy tag/logic component
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CEnemy {
    pub speed: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Default)]
pub struct Input {
    pub keys_pressed: HashSet<u32>,
    pub keys_just_pressed: HashSet<u32>,
}

impl Input {
    pub fn is_key_pressed(&self, key: u32) -> bool {
        self.keys_pressed.contains(&key)
    }
    pub fn is_key_just_pressed(&self, key: u32) -> bool {
        self.keys_just_pressed.contains(&key)
    }
}

pub trait GameLogic {
    fn on_load(&mut self, _world: &mut dyn std::any::Any) {
        println!("Game Plugin Loaded!");
    }
    fn update(&mut self, world: &mut dyn std::any::Any, input: &Input, dt: f32);
    fn on_unload(&mut self, _world: &mut dyn std::any::Any) {
        println!("Game Plugin Unloaded!");
    }
}
