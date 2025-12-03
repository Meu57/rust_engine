// crates/engine_core/src/input/map.rs
use std::collections::HashMap;
use winit::keyboard::KeyCode;
use engine_shared::ActionId;

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