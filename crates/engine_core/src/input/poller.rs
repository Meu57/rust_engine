// crates/engine_core/src/input/poller.rs

use crate::input::arbiter::ActionSignal;
use crate::input::{Arbiter, InputMap};
use engine_shared::input_types::PriorityLayer;
use winit::event::{KeyEvent, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

/// Low-level input collector that tracks active physical keys.
/// This keeps raw device state out of App / PlatformRunner.
pub struct InputPoller {
    active_keys: Vec<KeyCode>,
}

impl InputPoller {
    pub fn new() -> Self {
        Self {
            active_keys: Vec::new(),
        }
    }

    /// Process a single winit WindowEvent and update internal key state.
    pub fn handle_event(&mut self, event: &WindowEvent) {
        if let WindowEvent::KeyboardInput { event: key_event, .. } = event {
            self.handle_keyboard_input(key_event);
        }
    }

    fn handle_keyboard_input(&mut self, key_event: &KeyEvent) {
        if let PhysicalKey::Code(keycode) = key_event.physical_key {
            match key_event.state {
                winit::event::ElementState::Pressed => {
                    if !self.active_keys.contains(&keycode) {
                        self.active_keys.push(keycode);
                    }
                }
                winit::event::ElementState::Released => {
                    self.active_keys.retain(|&k| k != keycode);
                }
            }
        }
    }

    /// Returns true if a given physical key is currently pressed.
    /// Used to implement engine-level Reflex tests (e.g., P key) without
    /// leaking active_keys out of this struct.
    pub fn is_key_active(&self, key: KeyCode) -> bool {
        self.active_keys.contains(&key)
    }

    /// Sync raw physical state into the high-level Arbiter using the InputMap.
    /// This clears the Arbiter first, as in the original App::run.
    pub fn synchronize_with_arbiter(&self, arbiter: &mut Arbiter, input_map: &InputMap) {
        arbiter.clear();

        for &key in &self.active_keys {
            let physical = PhysicalKey::Code(key);
            if let Some(action_id) =
                input_map.map_signal_to_intent(Some(key), physical)
            {
                arbiter.add_action(ActionSignal {
                    layer: PriorityLayer::Control,
                    action_id,
                    active: true,
                });
            }
        }
    }
}
