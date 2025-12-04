// crates/engine_shared/src/plugin_api.rs
use core::ffi::{c_char, c_void};
use crate::input_types::{ActionId, InputState};

/// Host-provided function table (V-table) exposed to plugins.
/// #[repr(C)] guarantees the layout is preserved across FFI boundaries.
#[repr(C)]
pub struct HostInterface {
    /// Resolve action name -> ActionId
    pub get_action_id: extern "C" fn(name_ptr: *const u8, name_len: usize) -> ActionId,
    /// Optional logging callback from plugin -> host
    pub log: Option<extern "C" fn(msg: *const c_char)>,
    /// Host-provided spawn function: host receives opaque world_ptr and does the allocation.
    pub spawn_enemy: extern "C" fn(world_ptr: *mut c_void, x: f32, y: f32),
}

/// Plugin -> Host function table returned by the plugin on creation.
/// This manual V-table replaces `dyn GameLogic` to ensure ABI stability.
#[repr(C)]
pub struct PluginApi {
    /// Opaque plugin state pointer (owned by the plugin).
    pub state: *mut c_void,
    /// Called once after plugin creation: (state, world_ptr, host_interface)
    pub on_load: extern "C" fn(*mut c_void, *mut c_void, &HostInterface),
    /// Called every frame: (state, world_ptr, input_state, dt)
    pub update: extern "C" fn(*mut c_void, *mut c_void, &InputState, f32),
    /// Optional unload hook: (state, world_ptr)
    pub on_unload: extern "C" fn(*mut c_void, *mut c_void),
    /// Plugin-owned destructor: host calls this to let the plugin free its state.
    pub drop: extern "C" fn(*mut c_void),
}

/// Minimal trait used for in-process plugin style compatibility (kept for convenience)
pub trait GameLogic {
    fn on_load(&mut self, _world: &mut dyn std::any::Any, _host: &HostInterface) {}
    fn update(&mut self, _world: &mut dyn std::any::Any, _input: &InputState, _dt: f32) {}
    fn on_unload(&mut self, _world: &mut dyn std::any::Any) {}
}