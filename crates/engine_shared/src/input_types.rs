// crates/engine_shared/src/input_types.rs
use glam::Vec2;

/// Stable Integer ID for Actions (FFI-safe)
pub type ActionId = u32;
pub const ACTION_NOT_FOUND: ActionId = u32::MAX;

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
    pub digital_mask: u64,
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
        if (action_id as usize) >= 64 { return false; }
        (self.digital_mask & (1u64 << action_id)) != 0
    }

    pub fn get_axis(&self, axis_index: usize) -> f32 {
        if axis_index >= MAX_AXES { 0.0 } else { self.analog_axes[axis_index] }
    }
}