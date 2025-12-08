// crates/engine_ecs/src/world.rs

use std::any::{TypeId, type_name};
use std::collections::HashMap;

use crate::storage::{Storage, SparseSet};
use crate::entity::Entity;

pub struct World {
    entities: Vec<Entity>,
    // Map Component Type -> Storage
    components: HashMap<TypeId, Box<dyn Storage>>,
    free_indices: Vec<u32>,
    generations: Vec<u32>,
}

impl World {
    pub fn new() -> Self {
        Self {
            entities: Vec::new(),
            components: HashMap::new(),
            free_indices: Vec::new(),
            generations: Vec::new(),
        }
    }

    /// Register a component type with the world.
    /// This MUST be called exactly once per component type.
    pub fn register_component<T: 'static>(&mut self) {
        let type_id = TypeId::of::<T>();

        if self.components.contains_key(&type_id) {
            panic!(
                "Component {} registered twice. \
                 Ensure you only call world.register_component::<{}>() once.",
                type_name::<T>(),
                type_name::<T>(),
            );
        }

        self.components
            .insert(type_id, Box::new(SparseSet::<T>::new()));
    }

    pub fn spawn(&mut self) -> Entity {
        let index = if let Some(idx) = self.free_indices.pop() {
            idx
        } else {
            self.generations.push(0);
            (self.generations.len() - 1) as u32
        };

        let generation = self.generations[index as usize];
        let entity = Entity::new(index, generation);
        self.entities.push(entity);
        entity
    }

    /// STRICT MODE: adding a component to an unregistered type is a hard error.
    pub fn add_component<T: 'static>(&mut self, entity: Entity, component: T) {
        use std::collections::hash_map::Entry;

        let type_id = TypeId::of::<T>();

        match self.components.entry(type_id) {
            Entry::Occupied(mut occ) => {
                let storage = occ.get_mut();
                let sparse_set = storage
                    .as_any_mut()
                    .downcast_mut::<SparseSet<T>>()
                    .unwrap_or_else(|| {
                        panic!(
                            "Component storage type mismatch for {}. \
                             Storage was created for a different concrete type.",
                            type_name::<T>(),
                        )
                    });

                sparse_set.insert(entity, component);
            }
            Entry::Vacant(_) => {
                // LOUD FAILURE: this is exactly what we want in a serious engine.
                panic!(
                    "Component {} was not registered! \
                     Call world.register_component::<{}>() during setup (e.g. scene::setup_default_world).",
                    type_name::<T>(),
                    type_name::<T>(),
                );
            }
        }
    }

    /// Returns a shared reference to the component `T` for `entity`, or `None` if not present.
    pub fn get_component<T: 'static>(&self, entity: Entity) -> Option<&T> {
        let type_id = TypeId::of::<T>();
        if let Some(storage) = self.components.get(&type_id) {
            if let Some(sparse_set) = storage.as_any().downcast_ref::<SparseSet<T>>() {
                return sparse_set.get(entity);
            }
        }
        None
    }

    /// Read-only access to the full storage of a component type.
    pub fn query<T: 'static>(&self) -> Option<&SparseSet<T>> {
        let type_id = TypeId::of::<T>();
        self.components
            .get(&type_id)
            .and_then(|boxed| boxed.as_any().downcast_ref::<SparseSet<T>>())
    }

    /// Mutable access to the full storage of a component type.
    pub fn query_mut<T: 'static>(&mut self) -> Option<&mut SparseSet<T>> {
        let type_id = TypeId::of::<T>();
        self.components
            .get_mut(&type_id)
            .and_then(|boxed| boxed.as_any_mut().downcast_mut::<SparseSet<T>>())
    }
}
