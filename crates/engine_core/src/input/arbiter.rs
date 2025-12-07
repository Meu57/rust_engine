// crates/engine_core/src/input/arbiter.rs

use glam::Vec2;

use engine_shared::input_types::{
    ActionId,
    InputState,
    PriorityLayer,
    canonical_actions,
};

pub mod channels {
    use engine_shared::input_types::canonical_actions::*;

    // Mask for all movement bits based on shared Action IDs
    pub const MASK_MOVE: u64 =
        (1 << MOVE_UP) | (1 << MOVE_DOWN) | (1 << MOVE_LEFT) | (1 << MOVE_RIGHT);
    pub const MASK_ALL: u64 = !0;
}

#[derive(Clone, Copy)]
pub struct LayerConfig {
    pub layer: PriorityLayer,
    pub allowed_mask_when_active: u64,
    pub lock_on_activation: bool,
    pub lock_frames_on_activation: u32,
}

#[derive(Default, Clone, Copy)]
pub struct LayerRuntimeState {
    pub lock_frames_remaining: u32,
    pub locked_permission_mask: u64,
}

pub struct MovementSignal {
    pub layer: PriorityLayer,
    pub vector: Vec2,
    pub weight: f32,
}

pub struct ActionSignal {
    pub layer: PriorityLayer,
    pub action_id: ActionId,
    pub active: bool,
}

pub struct Arbiter {
    pub layer_configs: Vec<LayerConfig>,
    pub layer_state: Vec<LayerRuntimeState>,
    pub move_signals: Vec<MovementSignal>,
    pub action_signals: Vec<ActionSignal>,
    pub deadzone: f32,
}

impl Default for Arbiter {
    fn default() -> Self {
        Self {
            layer_configs: Vec::new(),
            layer_state: Vec::new(),
            move_signals: Vec::new(),
            action_signals: Vec::new(),
            deadzone: 0.1,
        }
    }
}

impl Arbiter {
    pub fn new(layer_configs: Vec<LayerConfig>, deadzone: f32) -> Self {
        let layer_state = vec![LayerRuntimeState::default(); layer_configs.len()];
        Self {
            layer_configs,
            layer_state,
            move_signals: Vec::new(),
            action_signals: Vec::new(),
            deadzone,
        }
    }

    pub fn clear(&mut self) {
        self.move_signals.clear();
        self.action_signals.clear();
    }

    pub fn add_movement(&mut self, signal: MovementSignal) {
        if signal.vector != Vec2::ZERO {
            self.move_signals.push(signal);
        }
    }

    pub fn add_action(&mut self, signal: ActionSignal) {
        self.action_signals.push(signal);
    }

    pub fn resolve(&mut self) -> InputState {
        let mut state = InputState::default();

        // FIRST PASS: compute activity per layer using only immutable borrows.
        let layer_activities: Vec<bool> = self
            .layer_configs
            .iter()
            .map(|cfg| self.layer_has_activity(cfg.layer))
            .collect();

        // SECOND PASS: mutate layer_state + compute global_permission.
        let mut global_permission: u64 = channels::MASK_ALL;

        for (idx, config) in self.layer_configs.iter().enumerate() {
            let runtime = &mut self.layer_state[idx];
            let layer_active = layer_activities[idx];

            let layer_mask = if runtime.lock_frames_remaining > 0 {
                runtime.lock_frames_remaining -= 1;
                runtime.locked_permission_mask
            } else if layer_active {
                let mask = config.allowed_mask_when_active;
                if config.lock_on_activation && config.lock_frames_on_activation > 0 {
                    runtime.lock_frames_remaining = config.lock_frames_on_activation;
                    runtime.locked_permission_mask = mask;
                }
                mask
            } else {
                channels::MASK_ALL
            };

            global_permission &= layer_mask;
        }

        // Resolve analog
        let final_vector = self.resolve_movement(global_permission);
        state.analog_axes[0] = final_vector.x;
        state.analog_axes[1] = final_vector.y;

        // Resolve digital
        let mut digital_requests: u64 = 0;
        for sig in &self.action_signals {
            let bit_index = sig.action_id as u32;
            if bit_index < 64 && sig.active {
                digital_requests |= 1u64 << bit_index;
            }
        }
        state.digital_mask = digital_requests & global_permission;

        state
    }

    fn layer_has_activity(&self, layer: PriorityLayer) -> bool {
        self.move_signals
            .iter()
            .any(|s| s.layer == layer && s.vector != Vec2::ZERO)
            || self
                .action_signals
                .iter()
                .any(|s| s.layer == layer && s.active)
    }

    fn resolve_movement(&self, global_permission: u64) -> Vec2 {
        use engine_shared::input_types::canonical_actions::*;

        if self.move_signals.is_empty() {
            return Vec2::ZERO;
        }

        let mut winning_layer: Option<PriorityLayer> = None;
        for cfg in &self.layer_configs {
            if self
                .move_signals
                .iter()
                .any(|s| s.layer == cfg.layer && s.vector != Vec2::ZERO)
            {
                winning_layer = Some(cfg.layer);
                break;
            }
        }

        let Some(layer) = winning_layer else { return Vec2::ZERO; };
        let mut raw = Vec2::ZERO;
        for sig in &self.move_signals {
            if sig.layer == layer {
                raw += sig.vector * sig.weight;
            }
        }

        let mut final_vec = raw;
        // Clamp axis components if specific direction bits are suppressed
        if (global_permission & (1 << MOVE_RIGHT)) == 0 && final_vec.x > 0.0 {
            final_vec.x = 0.0;
        }
        if (global_permission & (1 << MOVE_LEFT)) == 0 && final_vec.x < 0.0 {
            final_vec.x = 0.0;
        }
        if (global_permission & (1 << MOVE_UP)) == 0 && final_vec.y > 0.0 {
            final_vec.y = 0.0;
        }
        if (global_permission & (1 << MOVE_DOWN)) == 0 && final_vec.y < 0.0 {
            final_vec.y = 0.0;
        }

        if final_vec.x.abs() < self.deadzone {
            final_vec.x = 0.0;
        }
        if final_vec.y.abs() < self.deadzone {
            final_vec.y = 0.0;
        }

        let len_sq = final_vec.length_squared();
        if len_sq > 1.0 {
            final_vec /= len_sq.sqrt();
        }
        final_vec
    }
}
