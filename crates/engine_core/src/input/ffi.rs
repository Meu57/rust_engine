// crates/engine_core/src/input/ffi.rs
use std::sync::{Mutex, OnceLock};
use engine_shared::{ActionId, ACTION_NOT_FOUND};
use super::registry::ActionRegistry;

pub static GLOBAL_REGISTRY: OnceLock<Mutex<ActionRegistry>> = OnceLock::new();

pub extern "C" fn host_get_action_id(name_ptr: *const u8, name_len: usize) -> ActionId {
    unsafe {
        if name_ptr.is_null() || name_len == 0 {
            return ACTION_NOT_FOUND;
        }
        let slice = std::slice::from_raw_parts(name_ptr, name_len);
        if let Ok(name) = std::str::from_utf8(slice) {
            if let Some(mutex) = GLOBAL_REGISTRY.get() {
                if let Ok(reg) = mutex.lock() {
                    return reg.get_id(name).unwrap_or(ACTION_NOT_FOUND);
                }
            }
        }
    }
    ACTION_NOT_FOUND
}