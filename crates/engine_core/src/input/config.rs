// crates/engine_core/src/input/config.rs

use crate::input::arbiter::{channels, LayerConfig};
use crate::input::{ActionRegistry, InputMap};
use engine_shared::input_types::{canonical_actions, PriorityLayer};
use winit::keyboard::KeyCode;

/// Centralized defaults for input configuration.
/// This keeps App::new small and makes it easy to tweak or mod.
pub struct InputDefaults;

impl InputDefaults {
    /// Registers canonical movement actions and their default key bindings.
    ///
    /// IMPORTANT: We register movement actions first so their numeric IDs
    /// match canonical_actions::{MOVE_UP, MOVE_DOWN, MOVE_LEFT, MOVE_RIGHT}.
    pub fn setup(registry: &mut ActionRegistry, input_map: &mut InputMap) {
        // 1. Register canonical movement actions
        let move_up = registry.register("MoveUp");
        let move_down = registry.register("MoveDown");
        let move_left = registry.register("MoveLeft");
        let move_right = registry.register("MoveRight");

        // Verify alignment with canonical IDs (debug-only to avoid panics in Release).
        debug_assert_eq!(move_up, canonical_actions::MOVE_UP);
        debug_assert_eq!(move_down, canonical_actions::MOVE_DOWN);
        debug_assert_eq!(move_left, canonical_actions::MOVE_LEFT);
        debug_assert_eq!(move_right, canonical_actions::MOVE_RIGHT);

        // 2. Default WASD bindings
        input_map.bind_logical(KeyCode::KeyW, move_up);
        input_map.bind_logical(KeyCode::KeyS, move_down);
        input_map.bind_logical(KeyCode::KeyA, move_left);
        input_map.bind_logical(KeyCode::KeyD, move_right);
    }

    /// Default Arbiter layer configuration, matching the Reflex / Cutscene /
    /// Control / Ambient layering and veto behavior.
    pub fn default_arbiter_layers() -> Vec<LayerConfig> {
        vec![
            LayerConfig {
                layer: PriorityLayer::Reflex,
                allowed_mask_when_active: 0, // Block everything
                lock_on_activation: true,
                lock_frames_on_activation: 30,
            },
            LayerConfig {
                layer: PriorityLayer::Cutscene,
                allowed_mask_when_active: 0,
                lock_on_activation: false,
                lock_frames_on_activation: 0,
            },
            LayerConfig {
                layer: PriorityLayer::Control,
                allowed_mask_when_active: channels::MASK_ALL,
                lock_on_activation: false,
                lock_frames_on_activation: 0,
            },
            LayerConfig {
                layer: PriorityLayer::Ambient,
                allowed_mask_when_active: channels::MASK_ALL,
                lock_on_activation: false,
                lock_frames_on_activation: 0,
            },
        ]
    }
}
