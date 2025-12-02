use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt;

mod storage;
pub use storage::SparseSet;
use storage::Storage;

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

    pub fn register_component<T: 'static>(&mut self) {
        self.components.insert(
            TypeId::of::<T>(),
            Box::new(SparseSet::<T>::new()),
        );
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

    // --- Updated: add_component creates storage on-demand to avoid runtime panic ---
    pub fn add_component<T: 'static>(&mut self, entity: Entity, component: T) {
        use std::collections::hash_map::Entry;
        let type_id = TypeId::of::<T>();

        match self.components.entry(type_id) {
            Entry::Occupied(mut occ) => {
                if let Some(sparse_set) = occ.get_mut().as_any_mut().downcast_mut::<SparseSet<T>>() {
                    sparse_set.insert(entity, component);
                } else {
                    panic!("Component storage exists but has unexpected concrete type");
                }
            }
            Entry::Vacant(vac) => {
                let mut set = SparseSet::<T>::new();
                set.insert(entity, component);
                vac.insert(Box::new(set));
            }
        }
    }

    // --- NEW: get_component ---
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

    // Get a reference to a specific component storage
    pub fn query<T: 'static>(&self) -> Option<&SparseSet<T>> {
        let type_id = TypeId::of::<T>();
        self.components.get(&type_id)
            .and_then(|boxed| boxed.as_any().downcast_ref::<SparseSet<T>>())
    }

    // Get a MUTABLE reference (for modifying data)
    pub fn query_mut<T: 'static>(&mut self) -> Option<&mut SparseSet<T>> {
        let type_id = TypeId::of::<T>();
        self.components.get_mut(&type_id)
            .and_then(|boxed| boxed.as_any_mut().downcast_mut::<SparseSet<T>>())
    }
}

// A unique identifier for an entity.
// Bits 0-31: Index (The slot in the array)
// Bits 32-63: Generation (The version of this slot)
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Entity {
    id: u64,
}

impl Entity {
    const INDEX_MASK: u64 = 0xFFFFFFFF;
    const GENERATION_SHIFT: u64 = 32;

    pub fn new(index: u32, generation: u32) -> Self {
        let id = (index as u64) | ((generation as u64) << Self::GENERATION_SHIFT);
        Self { id }
    }

    pub fn index(&self) -> usize {
        (self.id & Self::INDEX_MASK) as usize
    }

    pub fn generation(&self) -> u32 {
        (self.id >> Self::GENERATION_SHIFT) as u32
    }
}

impl fmt::Debug for Entity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Entity({}:{})", self.index(), self.generation())
    }
}
