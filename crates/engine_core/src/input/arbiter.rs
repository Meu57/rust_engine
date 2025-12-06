// crates/engine_core/src/input/arbiter.rs
use glam::Vec2;
use engine_shared::{
    ActionSignal, InputState, MovementSignal, PriorityLayer,
};

// Define Action Bitflags (Masks)
// These map to specific bits in the InputState.digital_mask
// Ensure these match your Action IDs (0 = Up, 1 = Down, etc.) or are generic categories.
// For the Arbiter, we often group them by INTENT.
pub mod channels {
    pub const MOVE: u64   = 0b0000_1111; // IDs 0,1,2,3 (Up, Down, Left, Right)
    pub const LOOK: u64   = 0b0000_0000; // Reserved
    pub const ACTION: u64 = 0b0001_0000; // ID 4
    pub const PAUSE: u64  = 0b0010_0000; // ID 5
    pub const ALL: u64    = u64::MAX;
}

#[derive(Default)]
pub struct Arbiter {
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

        // ---------------------------------------------------------
        // 1. Resolve Movement (Winner Takes All)
        // ---------------------------------------------------------
        // We keep this behavior: Only ONE layer should drive the character's velocity.
        let mut winning_move_layer = PriorityLayer::Ambient;
        
        // Find the highest priority layer that is actually requesting movement
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

        // Normalize to prevent "1000.0" speed bugs if multiple signals add up
        if final_vector.length_squared() > 1.0 {
            final_vector = final_vector.normalize();
        }

        state.analog_axes[0] = final_vector.x;
        state.analog_axes[1] = final_vector.y;

        // ---------------------------------------------------------
        // 2. Resolve Actions (Cumulative Priority Stack)
        // ---------------------------------------------------------
        // Logic: Iterate Top -> Bottom.
        // Maintain a 'suppressed_mask'. Higher layers add to this mask to block lower layers.
        
        let mut global_suppression_mask: u64 = 0;

        for &layer in &[
            PriorityLayer::Reflex,
            PriorityLayer::Cutscene,
            PriorityLayer::Control,
            // Ambient is usually implicit/lowest, handled by default state
        ] {
            // A. Build the Request Mask for THIS layer
            let mut layer_request_mask: u64 = 0;
            for s in &self.action_signals {
                if s.layer == layer && s.active {
                    if (s.action_id as usize) < 64 {
                        layer_request_mask |= 1u64 << s.action_id;
                    }
                }
            }

            // B. Apply Suppression from Higher Layers
            // We only allow bits that have NOT been suppressed yet.
            let allowed_actions = layer_request_mask & !global_suppression_mask;
            
            // Add allowed actions to final state
            state.digital_mask |= allowed_actions;

            // C. Determine what THIS layer wants to suppress for layers below it.
            // NOTE: Ideally, 'ActionSignal' would carry suppression info.
            // For now, we define architectural rules here (The "Policy"):
            let layer_suppression = match layer {
                // If Reflex is active (e.g. Damage), it suppresses Movement but allows Pause
                PriorityLayer::Reflex => if layer_request_mask != 0 { 
                    channels::MOVE | channels::ACTION 
                } else { 0 },
                
                // Cutscenes suppress Movement and Action, but allow System (Pause)
                PriorityLayer::Cutscene => if layer_request_mask != 0 {
                    channels::MOVE | channels::ACTION
                } else { 0 },

                // Control layer generally doesn't suppress Ambient, but it could.
                PriorityLayer::Control => 0, 
                
                _ => 0,
            };

            // Add this layer's suppression to the global mask for the next iteration
            global_suppression_mask |= layer_suppression;
        }

        state
    }
}