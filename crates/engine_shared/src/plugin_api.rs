// crates/engine_shared/src/plugin_api.rs
use core::ffi::{c_char, c_void};
use crate::input_types::{ActionId, InputState};

// ==================================================================================
// 1. OPAQUE HANDLE (The "Firewall")
// ==================================================================================

/// Represents the Host's internal state (World, Resources, etc.).
/// 
/// PROPERTIES:
/// 1. Opaque: The plugin cannot see the fields (size is 0), so it cannot access memory directly.
/// 2. Type-Safe: It is a distinct type from `*mut c_void`, preventing accidental pointer mixing.
/// 3. !Send/!Sync: PhantomData ensures this handle stays on the main thread.
#[repr(C)]
pub struct HostContext {
    _data: [u8; 0],
    _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
}

// ==================================================================================
// 2. STRUCTURAL HASHING (The "Handshake")
// ==================================================================================

/// Helper to calculate a stable hash of a struct's layout.
/// In a real engine, this would be a derive macro (#[derive(StableLayout)]).
/// Here, we manually implement a simple FNV-1a hash for demonstration.
pub fn calculate_layout_hash(type_name: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    for b in type_name.bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(PRIME);
    }
    // In a real implementation, we would hash fields and alignments here.
    // For now, we hash the name to simulate "checking the definition".
    hash
}

// ==================================================================================
// 3. HOST INTERFACE
// ==================================================================================

#[repr(C)]
pub struct HostInterface {
    pub get_action_id: extern "C" fn(name_ptr: *const u8, name_len: usize) -> ActionId,
    pub log: Option<extern "C" fn(msg: *const c_char)>,
    
    // SAFETY FIX: Now takes `*mut HostContext` instead of `*mut c_void`
    pub spawn_enemy: extern "C" fn(ctx: *mut HostContext, x: f32, y: f32),
}

// ==================================================================================
// 4. PLUGIN API
// ==================================================================================

#[repr(C)]
pub struct PluginApi {
    pub state: *mut c_void,
    
    // Lifecycle methods now use `HostContext`
    pub on_load: extern "C" fn(*mut c_void, *mut HostContext, &HostInterface),
    pub update: extern "C" fn(*mut c_void, *mut HostContext, &InputState, f32),
    pub on_unload: extern "C" fn(*mut c_void, *mut HostContext),
    pub drop: extern "C" fn(*mut c_void),

    // NEW: The Safety Check
    // The Host calls this immediately after loading. 
    // If the hash doesn't match the Host's expectation, the load is aborted.
    pub get_layout_hash: extern "C" fn() -> u64, 
}