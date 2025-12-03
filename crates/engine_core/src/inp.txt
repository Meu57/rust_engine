use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use glam::Vec2;
use winit::keyboard::KeyCode;

use engine_shared::{
    ActionId, ActionSignal, InputState, MovementSignal, PriorityLayer, ACTION_NOT_FOUND,
};

// --- 1. ACTION REGISTRY ---
#[derive(Default, Clone)]
pub struct ActionRegistry {
    name_to_id: HashMap<String, ActionId>,
    next_id: ActionId,
}

impl ActionRegistry {
    pub fn register(&mut self, name: &str) -> ActionId {
        if let Some(&id) = self.name_to_id.get(name) {
            return id;
        }
        let id = self.next_id;
        self.name_to_id.insert(name.to_string(), id);
        self.next_id = self.next_id.wrapping_add(1);
        id
    }

    pub fn get_id(&self, name: &str) -> Option<ActionId> {
        self.name_to_id.get(name).copied()
    }
}

// --- 2. INPUT MAPPING ---
#[derive(Default)]
pub struct InputMap {
    key_bindings: HashMap<KeyCode, ActionId>,
}

impl InputMap {
    pub fn bind(&mut self, key: KeyCode, action: ActionId) {
        self.key_bindings.insert(key, action);
    }
    pub fn map_signal_to_intent(&self, key: KeyCode) -> Option<ActionId> {
        self.key_bindings.get(&key).copied()
    }
}

// --- 3. THE ARBITER ---
#[derive(Default)]
pub struct Arbiter {
    // Made public for inspector GUI access
    pub move_signals: Vec<MovementSignal>,
    pub action_signals: Vec<ActionSignal>,
}

impl Arbiter {
    pub fn clear(&mut self) {
        self.move_signals.clear();
        self.action_signals.clear();
    }

    pub fn add_movement(&mut self, signal: MovementSignal) {
        self.move_signals.push(signal);
    }

    pub fn add_action(&mut self, signal: ActionSignal) {
        self.action_signals.push(signal);
    }

    pub fn resolve(&self) -> InputState {
        let mut state = InputState::default();

        // A. Resolve Movement
        let mut winning_move_layer = PriorityLayer::Ambient;
        for &layer in &[
            PriorityLayer::Reflex,
            PriorityLayer::Cutscene,
            PriorityLayer::Control,
        ] {
            let has_signal = self.move_signals.iter().any(|s| s.layer == layer);
            if has_signal {
                winning_move_layer = layer;
                break;
            }
        }

        let mut final_vector = Vec2::ZERO;
        for s in &self.move_signals {
            if s.layer == winning_move_layer {
                final_vector += s.vector * s.weight;
            }
        }

        if final_vector.length_squared() > 1.0 {
            final_vector = final_vector.normalize();
        }

        state.analog_axes[0] = final_vector.x;
        state.analog_axes[1] = final_vector.y;

        // B. Resolve Actions (Digital)
        let mut winning_action_layer = PriorityLayer::Ambient;
        for &layer in &[
            PriorityLayer::Reflex,
            PriorityLayer::Cutscene,
            PriorityLayer::Control,
        ] {
            let has_signal = self.action_signals.iter().any(|s| s.layer == layer);
            if has_signal {
                winning_action_layer = layer;
                break;
            }
        }

        for s in &self.action_signals {
            if s.layer == winning_action_layer && s.active {
                if (s.action_id as usize) < 64 {
                    state.digital_mask |= 1u64 << s.action_id;
                }
            }
        }

        state
    }
}

// --- 4. GLOBAL ACCESS (FFI) ---
pub static GLOBAL_REGISTRY: OnceLock<Mutex<ActionRegistry>> = OnceLock::new();

pub extern "C" fn host_get_action_id(name_ptr: *const u8, name_len: usize) -> ActionId {
    unsafe {
        if name_ptr.is_null() || name_len == 0 {
            return ACTION_NOT_FOUND;
        }
        let slice = std::slice::from_raw_parts(name_ptr, name_len);
        if let Ok(name) = std::str::from_utf8(slice) {
            if let Some(mutex) = GLOBAL_REGISTRY.get() {
                if let Ok(reg) = mutex.lock() {
                    return reg.get_id(name).unwrap_or(ACTION_NOT_FOUND);
                }
            }
        }
    }
    ACTION_NOT_FOUND
}