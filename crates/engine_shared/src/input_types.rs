// crates/engine_shared/src/input_types.rs
//! Compact, FFI-friendly input types used by host <-> plugin and for networking/replay.

use glam::Vec2;

/// Stable Integer ID for Actions (FFI-safe)
pub type ActionId = u32;
pub const ACTION_NOT_FOUND: ActionId = u32::MAX;

/// Canonical Action IDs for core movement.
/// App must register these first to ensure they get IDs 0..3.
pub mod canonical_actions {
    use super::ActionId;
    pub const MOVE_UP: ActionId    = 0;
    pub const MOVE_DOWN: ActionId  = 1;
    pub const MOVE_LEFT: ActionId  = 2;
    pub const MOVE_RIGHT: ActionId = 3;
}

/// Maximum number of analog axes we expose across FFI
pub const MAX_AXES: usize = 8;

/// The Priority Stack for Subsumption Architecture
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PriorityLayer {
    Reflex = 0,
    Cutscene = 1,
    Control = 2,
    Ambient = 3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MovementSignal {
    pub layer: PriorityLayer,
    pub vector: Vec2,
    pub weight: f32,
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

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ActionSignal {
    pub layer: PriorityLayer,
    pub action_id: ActionId,
    pub active: bool,
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
    /// Bitmask for up to 64 digital actions (result of arbitration).
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
    pub fn is_active(&self, action_id: ActionId) -> bool {
        if (action_id as usize) >= 64 {
            return false;
        }
        (self.digital_mask & (1u64 << action_id)) != 0
    }

    pub fn get_axis(&self, axis_index: usize) -> f32 {
        if axis_index >= MAX_AXES {
            0.0
        } else {
            self.analog_axes[axis_index]
        }
    }
}

/// Compact per-frame input snapshot for deterministic replay/netcode.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct FrameInputState {
    pub tick: u64,
    pub actions: u64,
    pub move_vector: [i16; 2],
    pub rng_seed: u64,
}

impl FrameInputState {
    pub fn from_state(tick: u64, seed: u64, state: &InputState) -> Self {
        let scale = 1000.0_f32;
        let raw_x = (state.analog_axes[0] * scale).round();
        let raw_y = (state.analog_axes[1] * scale).round();

        Self {
            tick,
            actions: state.digital_mask,
            move_vector: [clamp_i16(raw_x as i64), clamp_i16(raw_y as i64)],
            rng_seed: seed,
        }
    }
}

fn clamp_i16(v: i64) -> i16 {
    if v > i16::MAX as i64 {
        i16::MAX
    } else if v < i16::MIN as i64 {
        i16::MIN
    } else {
        v as i16
    }
}
