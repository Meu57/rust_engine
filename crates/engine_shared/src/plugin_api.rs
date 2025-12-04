// crates/engine_shared/src/plugin_api.rs
use core::ffi::c_char;
use core::ffi::c_void;
use crate::input_types::{ActionId, InputState}; // <--- Imports from input_types

#[repr(C)]
pub struct HostInterface {
    pub get_action_id: extern "C" fn(name_ptr: *const u8, name_len: usize) -> ActionId,
    pub log: Option<extern "C" fn(msg: *const c_char)>,
    pub spawn_enemy: extern "C" fn(world_ptr: *mut c_void, x: f32, y: f32),
}

pub trait GameLogic {
    fn on_load(&mut self, _world: &mut dyn std::any::Any, _host: &HostInterface) { }
    fn update(&mut self, world: &mut dyn std::any::Any, input: &InputState, dt: f32);
    fn on_unload(&mut self, _world: &mut dyn std::any::Any) { }
}