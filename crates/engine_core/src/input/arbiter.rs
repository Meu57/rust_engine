// crates/engine_core/src/input/arbiter.rs
use glam::Vec2;
use engine_shared::{
    ActionSignal, InputState, MovementSignal, PriorityLayer,
};

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