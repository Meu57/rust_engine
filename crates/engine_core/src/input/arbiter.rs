use glam::Vec2;
use engine_shared::{
    ActionSignal, InputState, MovementSignal, PriorityLayer,
};

// Define Action Bitflags (Masks)
pub mod channels {
    pub const MOVE: u64   = 0b0000_1111; // IDs 0,1,2,3 (Up, Down, Left, Right)
    pub const LOOK: u64   = 0b0000_0000; // Reserved
    pub const ACTION: u64 = 0b0001_0000; // ID 4
    pub const PAUSE: u64  = 0b0010_0000; // ID 5
    pub const ALL: u64    = u64::MAX;
}

/// Per-layer persistent state for temporal locking.
/// Instead of using real time, we count in frames.
#[derive(Default)]
pub struct LayerState {
    pub lock_frames_remaining: u32,
    pub persistent_mask: u64,
}

#[derive(Default)]
pub struct Arbiter {
    pub move_signals: Vec<MovementSignal>,
    pub action_signals: Vec<ActionSignal>,
    pub layer_states: [LayerState; 4], // Reflex, Cutscene, Control, Ambient
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

    /// Resolve the final InputState for this frame.
    /// Uses per-frame temporal locking: certain layers (e.g. Reflex)
    /// can keep their suppression active for a few frames even after
    /// the original signal is gone.
    pub fn resolve(&mut self) -> InputState {
        let mut state = InputState::default();

        // ---------------------------------------------------------
        // 1. Resolve Movement (Winner Takes All)
        // ---------------------------------------------------------
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

        // ---------------------------------------------------------
        // 2. Resolve Actions (Layers + Temporal Locking)
        // ---------------------------------------------------------
        let mut global_suppression_mask: u64 = 0;

        let layers = [
            PriorityLayer::Reflex,
            PriorityLayer::Cutscene,
            PriorityLayer::Control,
            PriorityLayer::Ambient,
        ];

        for (idx, &layer) in layers.iter().enumerate() {
            // A. Build request mask for this layer from current signals
            let mut layer_request_mask: u64 = 0;
            for s in &self.action_signals {
                if s.layer == layer && s.active {
                    if (s.action_id as usize) < 64 {
                        layer_request_mask |= 1u64 << s.action_id;
                    }
                }
            }

            // --- TEMPORAL LOCKING: reuse previous suppression if still locked ---
            let layer_state = &mut self.layer_states[idx];

            if layer_state.lock_frames_remaining > 0 {
                layer_request_mask |= layer_state.persistent_mask;
                layer_state.lock_frames_remaining -= 1;
            }

            // B. Decide this layer's suppression policy
            let layer_suppression = match layer {
                // Reflex suppresses movement + actions while active/locked
                PriorityLayer::Reflex => {
                    if layer_request_mask != 0 {
                        channels::MOVE | channels::ACTION
                    } else {
                        0
                    }
                }
                // Cutscenes suppress movement and actions
                PriorityLayer::Cutscene => {
                    if layer_request_mask != 0 {
                        channels::MOVE | channels::ACTION
                    } else {
                        0
                    }
                }
                // Control normally doesn't suppress others
                PriorityLayer::Control => 0,
                _ => 0,
            };

            // If Reflex just became active this frame, start a short lock window.
            // At 60 FPS, 30 frames ~= 0.5 seconds.
            if layer == PriorityLayer::Reflex && layer_request_mask != 0 {
                if layer_state.lock_frames_remaining == 0 {
                    layer_state.lock_frames_remaining = 30;
                    layer_state.persistent_mask = layer_suppression;
                }
            }

            // C. Apply global suppression
            let allowed = layer_request_mask & !global_suppression_mask;
            state.digital_mask |= allowed;

            global_suppression_mask |= layer_suppression;
        }

        state
    }
}