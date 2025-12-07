// crates/engine_shared/src/plugin_api.rs

use core::ffi::{c_char, c_void};
use crate::input_types::{ActionId, InputState};

// ==================================================================================
// 1. CONSTANTS & ENUMS
// ==================================================================================

pub const SNAPSHOT_MAGIC_HEADER: u32 = 0xCAFEBABE;
pub const CURRENT_SCHEMA_HASH: u64 = 0x0123_4567_89AB_CDEF;
pub const CURRENT_STATE_VERSION: u32 = 1;

#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum FFIResult {
    Success        = 0,
    BufferTooSmall = 1,
    SchemaMismatch = 2,
    PanicDetected  = 3,
    Error          = 4,
}

// ==================================================================================
// 2. DATA STRUCTURES
// ==================================================================================

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct FFIBuffer {
    pub ptr: *mut u8,
    pub len: usize,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct StateEnvelope {
    pub magic_header: u32,
    pub state_version: u32,
    pub schema_hash: u64,
    pub payload_len: u64,
}

// ==================================================================================
// 3. HOST TYPES
// ==================================================================================

/// Opaque handle to the Host's World/Resources.
#[repr(C)]
pub struct HostContext {
    _data: [u8; 0],
    _marker: core::marker::PhantomData<(*mut u8, core::marker::PhantomPinned)>,
}

/// VTable of functions provided by the Host to the Plugin.
#[repr(C)]
pub struct HostInterface {
    pub get_action_id: extern "C" fn(name_ptr: *const u8, name_len: usize) -> ActionId,
    pub log: Option<extern "C" fn(msg: *const c_char)>,
    pub spawn_enemy: extern "C" fn(ctx: *mut HostContext, x: f32, y: f32),
}

// ==================================================================================
// 4. PLUGIN API (VTable)
// ==================================================================================

#[repr(C)]
pub struct PluginApi {
    pub state: *mut c_void,

    // Lifecycle using specific types
    pub on_load: extern "C" fn(
        state: *mut c_void,
        host_ctx: *mut HostContext,
        host_iface: *const HostInterface,
    ) -> FFIResult,

    pub on_update: extern "C" fn(
        state: *mut c_void,
        host_ctx: *mut HostContext,
        input: *const InputState,
        dt: f32,
    ) -> FFIResult,

    pub on_unload: extern "C" fn(state: *mut c_void, host_ctx: *mut HostContext) -> FFIResult,

    // State Management
    pub get_state_len: extern "C" fn(state: *mut c_void) -> usize,
    pub save_state: extern "C" fn(state: *mut c_void, buffer: FFIBuffer) -> FFIResult,
    pub load_state: extern "C" fn(state: *mut c_void, buffer: FFIBuffer) -> FFIResult,

    pub drop_state: extern "C" fn(state: *mut c_void),
    pub get_schema_hash: extern "C" fn() -> u64,
}
