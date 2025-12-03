use std::fmt;

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