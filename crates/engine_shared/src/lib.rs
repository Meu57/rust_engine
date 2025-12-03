// crates/engine_shared/src/lib.rs
#![allow(dead_code)]

use glam::{Vec2, Vec4};
use core::ffi::c_char;

/// Stable Integer ID for Actions (FFI-safe)
pub type ActionId = u32;
pub const ACTION_NOT_FOUND: ActionId = u32::MAX;

/// Maximum number of analog axes we expose across FFI
pub const MAX_AXES: usize = 8;

/// The Priority Stack for Subsumption Architecture
/// Lower value = Higher Priority (Layer 0 subsumes Layer 1)
/// Reference: Input_Handeling_0.pdf, Page 49
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PriorityLayer {
    /// Layer 0: Critical (Physics/Reflex)
    /// Absolute Override. E.g., Knockback, Stun.
    Reflex = 0,

    /// Layer 1: Scripts/Cutscenes
    /// Input Lock. E.g., Cinematics taking control.
    Cutscene = 1,

    /// Layer 2: Active Control (Player or AI)
    /// Standard gameplay input.
    Control = 2,

    /// Layer 3: Ambient (Idle)
    /// Fidgets, default animations.
    Ambient = 3,
}

/// An intermediate intent to move (before arbitration).
/// Systems write this to a buffer; the Arbiter reads it.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MovementSignal {
    pub layer: PriorityLayer,
    pub vector: Vec2, // Where we want to go
    pub weight: f32,  // 0.0 to 1.0 (For blending)
}

impl Default for MovementSignal {
    fn default() -> Self {
        Self {
            layer: PriorityLayer::Ambient,
            vector: Vec2::ZERO,
            weight: 0.0,
        }
    }
}

/// An intermediate intent to perform an action (before arbitration).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ActionSignal {
    pub layer: PriorityLayer,
    pub action_id: ActionId,
    pub active: bool, // Pressed / Released
}

impl Default for ActionSignal {
    fn default() -> Self {
        Self {
            layer: PriorityLayer::Ambient,
            action_id: ACTION_NOT_FOUND,
            active: false,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct InputState {
    /// Bitmask for up to 64 digital actions.
    /// If bit N set -> ActionId(N) is active.
    /// This is the RESULT of arbitration.
    pub digital_mask: u64,

    /// Fixed-size analog axes. Host maps an ActionId -> axis index.
    pub analog_axes: [f32; MAX_AXES],
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            digital_mask: 0,
            analog_axes: [0.0; MAX_AXES],
        }
    }
}

impl InputState {
    /// Safe check; returns false for out-of-range ids (including ACTION_NOT_FOUND).
    pub fn is_active(&self, action_id: ActionId) -> bool {
        if (action_id as usize) >= 64 { return false; }
        (self.digital_mask & (1u64 << action_id)) != 0
    }

    pub fn get_axis(&self, axis_index: usize) -> f32 {
        if axis_index >= MAX_AXES { 0.0 } else { self.analog_axes[axis_index] }
    }
}

/// Host -> Plugin vtable: extern "C" callable helpers passed during handshake.
#[repr(C)]
pub struct HostInterface {
    /// name_ptr + name_len -> ActionId (ACTION_NOT_FOUND if missing)
    pub get_action_id: extern "C" fn(name_ptr: *const u8, name_len: usize) -> ActionId,

    /// OPTIONAL: diagnostic helper; can be null if host doesn't support it.
    pub log: Option<extern "C" fn(msg: *const c_char)>,
}

/* ECS components (FFI-safe PODs) */

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

/* GameLogic trait — NOTE: not FFI; plugin exports factory that returns a boxed trait object */
pub trait GameLogic {
    /// Called once after plugin loaded. HostInterface supplied for negotiation.
    fn on_load(&mut self, _world: &mut dyn std::any::Any, _host: &HostInterface) { }

    /// Main update; receives FFI-safe InputState
    fn update(&mut self, world: &mut dyn std::any::Any, input: &InputState, dt: f32);

    fn on_unload(&mut self, _world: &mut dyn std::any::Any) { }
}