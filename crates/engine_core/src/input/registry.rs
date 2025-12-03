// crates/engine_core/src/input/registry.rs
use std::collections::HashMap;
use engine_shared::ActionId;

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