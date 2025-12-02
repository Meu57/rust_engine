// crates/engine_ecs/src/storage.rs
use crate::Entity;

// The trait allows us to treat different component storages generically
pub trait Storage {
    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

pub struct SparseSet<T> {
    pub dense: Vec<T>,          // Tightly packed data (Cache friendly!)
    pub entities: Vec<Entity>,  // The entity that owns the data at 'dense[i]'
    pub sparse: Vec<Option<usize>>, // Maps Entity Index -> Dense Index
}

impl<T: 'static> SparseSet<T> {
    pub fn new() -> Self {
        Self {
            dense: Vec::new(),
            entities: Vec::new(),
            sparse: Vec::new(),
        }
    }

    pub fn insert(&mut self, entity: Entity, value: T) {
        let index = entity.index();
        
        // Resize sparse array if the entity index is too big
        if index >= self.sparse.len() {
            self.sparse.resize(index + 1, None);
        }

        // If this entity already has this component, overwrite it
        if let Some(dense_index) = self.sparse[index] {
            self.dense[dense_index] = value;
            self.entities[dense_index] = entity;
        } else {
            // New component: Push to the end of dense
            let dense_index = self.dense.len();
            self.dense.push(value);
            self.entities.push(entity);
            self.sparse[index] = Some(dense_index);
        }
    }

    pub fn get(&self, entity: Entity) -> Option<&T> {
        let index = entity.index();
        if index < self.sparse.len() {
            if let Some(dense_index) = self.sparse[index] {
                // Check generation to ensure the entity is still alive!
                if self.entities[dense_index].generation() == entity.generation() {
                    return Some(&self.dense[dense_index]);
                }
            }
        }
        None
    }

    // --- Added Methods ---

    // Expose the raw data for linear iteration (The "D" in DOD)
    pub fn as_slice(&self) -> &[T] {
        &self.dense
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.dense
    }

    // Iterate over (Entity, Component) pairs
    pub fn iter(&self) -> impl Iterator<Item = (&Entity, &T)> {
        self.entities.iter().zip(self.dense.iter())
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&Entity, &mut T)> {
        self.entities.iter().zip(self.dense.iter_mut())
    }
}

// Boilerplate to allow dynamic typing of the storage
impl<T: 'static> Storage for SparseSet<T> {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
}