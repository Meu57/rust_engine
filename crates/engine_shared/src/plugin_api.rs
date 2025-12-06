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
// ===================================================================================

/// Helper to calculate a stable hash of a struct's layout.
/// In a real engine, this would be a derive macro (#[derive(StableLayout)]).
pub fn calculate_layout_hash(type_name: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x1000_0000_01b3;

    for b in type_name.bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(PRIME);
    }

    hash
}

// ==================================================================================
// 3. VERSIONED SNAPSHOT ENVELOPE
// ==================================================================================

/// Header written before the serialized MyGame state.
///
/// Layout is #[repr(C)] so the plugin can safely interpret bytes on load.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct SnapshotHeader {
    /// Helper to detect if we are reading garbage or a valid snapshot.
    pub magic: u64,

    /// Increment this when you change MyGame struct fields.
    pub version: u32,

    /// Structural/protocol hash (see calculate_layout_hash).
    pub layout_hash: u64,

    /// Length of the actual serialized payload following this header.
    pub data_len: usize,
}

// ==================================================================================
// 4. HOST INTERFACE
// ==================================================================================

#[repr(C)]
pub struct HostInterface {
    pub get_action_id: extern "C" fn(name_ptr: *const u8, name_len: usize) -> ActionId,
    pub log: Option<extern "C" fn(msg: *const c_char)>,

    // SAFETY: HostContext is opaque to the plugin.
    pub spawn_enemy: extern "C" fn(ctx: *mut HostContext, x: f32, y: f32),
}

// ==================================================================================
// 5. PLUGIN API (Host-Authoritative Snapshot, Strategy B)
// ==================================================================================

#[repr(C)]
pub struct PluginApi {
    /// Opaque pointer to plugin-owned game state (Box<MyGame> on plugin side).
    pub state: *mut c_void,

    /// on_load now accepts an optional snapshot buffer (ptr + len).
    ///
    /// * snapshot_ptr == null OR snapshot_len == 0  => "start fresh".
    /// * Returns true if load/restore was successful.
    pub on_load: extern "C" fn(
        *mut c_void,         // state
        *mut HostContext,    // host ctx (really &mut World)
        *const HostInterface,// host vtable
        *const u8,           // snapshot_ptr (nullable)
        usize,               // snapshot_len
    ) -> bool,

    pub update: extern "C" fn(
        *mut c_void,         // state
        *mut HostContext,    // host ctx
        *const InputState,   // input
        f32,                 // dt
    ),

    pub on_unload: extern "C" fn(*mut c_void, *mut HostContext),

    // Strategy B: Host allocates the buffer, plugin fills it.
    pub get_state_size: extern "C" fn(*mut c_void) -> usize,
    pub save_state: extern "C" fn(*mut c_void, *mut u8, usize) -> usize,

    /// Plugin-side destructor for `state`.
    pub drop: extern "C" fn(*mut c_void),

    /// Structural hash check; host can compare with its expectation.
    pub get_layout_hash: extern "C" fn() -> u64,
}