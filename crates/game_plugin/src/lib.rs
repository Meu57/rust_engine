// crates/game_plugin/src/lib.rs

mod systems;
mod state;
mod shims; // <--- The new module

use std::ffi::c_void;
use engine_shared::plugin_api::PluginApi;
use crate::state::MyGame;

// The main entry point required by the engine.
// It maps the PluginApi vtable to the functions in 'shims.rs'.
#[no_mangle]
pub extern "C" fn _create_game() -> PluginApi {
    let state = Box::into_raw(Box::new(MyGame::default())) as *mut c_void;

    PluginApi {
        state,
        on_load: shims::on_load,
        on_update: shims::on_update,
        on_unload: shims::on_unload,
        get_state_len: shims::get_state_len,
        save_state: shims::save_state,
        load_state: shims::load_state,
        drop_state: shims::drop_state,
        get_schema_hash: shims::get_hash,
        get_state_version: shims::get_state_version,
    }
}