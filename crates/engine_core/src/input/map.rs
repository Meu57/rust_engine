// crates/engine_core/src/input/map.rs
use std::collections::HashMap;
use winit::keyboard::{KeyCode, PhysicalKey};
use engine_shared::ActionId;

#[derive(Default)]
pub struct InputMap {
    /// Logical bindings: "Press the key labeled 'W'".
    /// Good for menus, typing, and non-directional actions (e.g. 'I' for Inventory).
    logical_bindings: HashMap<KeyCode, ActionId>,

    /// Physical bindings: "Press the key at this specific circuit board location".
    /// Good for movement (WASD), ensuring the hand position stays the same
    /// regardless of the user's keyboard layout (QWERTY/AZERTY).
    physical_bindings: HashMap<PhysicalKey, ActionId>,
}

impl InputMap {
    /// Bind a Logical Key (based on label).
    pub fn bind_logical(&mut self, key: KeyCode, action: ActionId) {
        self.logical_bindings.insert(key, action);
    }

    /// Bind a Physical Key (based on location/scancode).
    pub fn bind_physical(&mut self, key: PhysicalKey, action: ActionId) {
        self.physical_bindings.insert(key, action);
    }

    /// Resolve an Action ID from a raw input event.
    ///
    /// The paper specifies that the engine must support both interpretations.
    /// Here, we check Physical first (Positional Priority), then Logical (Label Priority).
    pub fn map_signal_to_intent(&self, logical: Option<KeyCode>, physical: PhysicalKey) -> Option<ActionId> {
        // 1. Check Physical Binding (Highest Priority for Movement)
        // If the user bound "Physical Location 17" (Top-Left Letter) to "Move Forward",
        // we respect that regardless of what is printed on the keycap.
        if let Some(&id) = self.physical_bindings.get(&physical) {
            return Some(id);
        }

        // 2. Check Logical Binding (Fallback/Label Priority)
        // If no physical binding exists, check if the key's label matches a binding.
        // Example: User pressed a key labeled "I", and we have "I" bound to Inventory.
        if let Some(code) = logical {
            if let Some(&id) = self.logical_bindings.get(&code) {
                return Some(id);
            }
        }

        None
    }

    /// Clear all bindings (useful for resetting configuration).
    pub fn clear(&mut self) {
        self.logical_bindings.clear();
        self.physical_bindings.clear();
    }
}