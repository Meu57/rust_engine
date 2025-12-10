// crates/game_plugin/src/lib.rs

mod systems;

use std::ffi::c_void;
use std::io::Cursor;

use bincode;
use engine_ecs::World;
use engine_shared::input_types::{ActionId, InputState, ACTION_NOT_FOUND};
use engine_shared::plugin_api::{
    FFIResult,
    FFIBuffer,
    HostContext,
    HostInterface,
    PluginApi,
    StateEnvelope,
    SNAPSHOT_MAGIC_HEADER,
    CURRENT_SCHEMA_HASH,
    CURRENT_STATE_VERSION,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct MyGame {
    pub spawn_timer: f32,

    #[serde(default)]
    pub score: u32,

    #[serde(skip)]
    pub actions: [ActionId; 4],

    #[serde(skip)]
    pub spawn_fn: Option<extern "C" fn(*mut HostContext, f32, f32)>,
}

impl Default for MyGame {
    fn default() -> Self {
        Self {
            spawn_timer: 2.0,
            score: 0,
            actions: [ACTION_NOT_FOUND; 4],
            spawn_fn: None,
        }
    }
}

impl MyGame {
    pub fn bind_host_resources(&mut self, host: &HostInterface) {
        self.actions[0] = (host.get_action_id)(b"MoveUp".as_ptr(), b"MoveUp".len());
        self.actions[1] = (host.get_action_id)(b"MoveDown".as_ptr(), b"MoveDown".len());
        self.actions[2] = (host.get_action_id)(b"MoveLeft".as_ptr(), b"MoveLeft".len());
        self.actions[3] = (host.get_action_id)(b"MoveRight".as_ptr(), b"MoveRight".len());
        self.spawn_fn = Some(host.spawn_enemy);
    }
}

fn catch_ffi_panic<F>(f: F) -> FFIResult
where
    F: FnOnce() -> FFIResult + std::panic::UnwindSafe,
{
    match std::panic::catch_unwind(f) {
        Ok(res) => res,
        Err(_) => FFIResult::PanicDetected,
    }
}

// ---------------------------------------------------------------------
// SHIMS
// ---------------------------------------------------------------------

extern "C" fn shim_on_load(
    state: *mut c_void,
    _ctx: *mut HostContext,
    iface: *const HostInterface,
) -> FFIResult {
    catch_ffi_panic(|| {
        if state.is_null() || iface.is_null() {
            return FFIResult::Error;
        }

        unsafe {
            let game = &mut *(state as *mut MyGame);
            let host = &*iface;
            game.bind_host_resources(host);
        }

        FFIResult::Success
    })
}

extern "C" fn shim_on_update(
    state: *mut c_void,
    ctx: *mut HostContext,
    input: *const InputState,
    dt: f32,
) -> FFIResult {
    catch_ffi_panic(|| {
        if state.is_null() || ctx.is_null() || input.is_null() {
            return FFIResult::Error;
        }

        unsafe {
            let game = &mut *(state as *mut MyGame);
            let world = &mut *(ctx as *mut World);
            let input = &*input;

            systems::player::update_player(world, input, dt, &game.actions);
            
            // --- NEW: Update Camera ---
            systems::camera::update_camera(world, dt);

            if let Some(spawn_fn) = game.spawn_fn {
                let ctx_ptr = world as *mut World as *mut HostContext;
                systems::enemy::spawn_enemies(spawn_fn, ctx_ptr, &mut game.spawn_timer, dt);
            }
        }

        FFIResult::Success
    })
}

// (The rest of this file (shim_on_unload, shim_save_state, etc.) is unchanged. 
// You can keep the existing code below this point.)

extern "C" fn shim_on_unload(_state: *mut c_void, _ctx: *mut HostContext) -> FFIResult {
    FFIResult::Success
}

extern "C" fn shim_get_state_len(state: *mut c_void) -> usize {
    if state.is_null() {
        return 0;
    }

    let game = unsafe { &*(state as *mut MyGame) };
    let payload = bincode::serialized_size(game).unwrap_or(0) as usize;

    std::mem::size_of::<StateEnvelope>() + payload
}

extern "C" fn shim_save_state(state: *mut c_void, buf: FFIBuffer) -> FFIResult {
    catch_ffi_panic(|| {
        if state.is_null() || buf.ptr.is_null() {
            return FFIResult::Error;
        }
        let game = unsafe { &*(state as *mut MyGame) };

        let payload_len = match bincode::serialized_size(game) {
            Ok(sz) => sz as usize,
            Err(_) => return FFIResult::Error,
        };

        let header_len = std::mem::size_of::<StateEnvelope>();
        let total_len = header_len + payload_len;

        if buf.len < total_len {
            return FFIResult::BufferTooSmall;
        }

        let envelope = StateEnvelope {
            magic_header: SNAPSHOT_MAGIC_HEADER,
            state_version: CURRENT_STATE_VERSION,
            schema_hash: CURRENT_SCHEMA_HASH,
            payload_len: payload_len as u64,
        };

        unsafe {
            // Copy envelope
            std::ptr::copy_nonoverlapping(
                &envelope as *const _ as *const u8,
                buf.ptr,
                header_len,
            );

            // Copy payload
            let payload_slice =
                std::slice::from_raw_parts_mut(buf.ptr.add(header_len), payload_len);
            let mut cursor = Cursor::new(payload_slice);

            if bincode::serialize_into(&mut cursor, game).is_err() {
                return FFIResult::Error;
            }
        }

        FFIResult::Success
    })
}

extern "C" fn shim_load_state(state: *mut c_void, buf: FFIBuffer) -> FFIResult {
    catch_ffi_panic(|| {
        if state.is_null() || buf.ptr.is_null() {
            return FFIResult::Error;
        }

        let game = unsafe { &mut *(state as *mut MyGame) };
        let header_len = std::mem::size_of::<StateEnvelope>();

        if buf.len < header_len {
            return FFIResult::Error;
        }

        // Copy envelope bytes into a properly aligned local
        let mut envelope = StateEnvelope {
            magic_header: 0,
            state_version: 0,
            schema_hash: 0,
            payload_len: 0,
        };

        unsafe {
            std::ptr::copy_nonoverlapping(
                buf.ptr as *const u8,
                &mut envelope as *mut StateEnvelope as *mut u8,
                header_len,
            );
        }

        if envelope.magic_header != SNAPSHOT_MAGIC_HEADER {
            return FFIResult::Error;
        }
        if envelope.schema_hash != CURRENT_SCHEMA_HASH {
            return FFIResult::SchemaMismatch;
        }

        let payload_len = envelope.payload_len as usize;
        if buf.len < header_len + payload_len {
            return FFIResult::Error;
        }

        unsafe {
            let payload_slice =
                std::slice::from_raw_parts(buf.ptr.add(header_len), payload_len);
            let mut cursor = Cursor::new(payload_slice);

            match bincode::deserialize_from(&mut cursor) {
                Ok(g) => {
                    *game = g;
                    FFIResult::Success
                }
                Err(_) => FFIResult::Error,
            }
        }
    })
}

extern "C" fn shim_drop_state(state: *mut c_void) {
    if !state.is_null() {
        unsafe { drop(Box::from_raw(state as *mut MyGame)); }
    }
}

extern "C" fn shim_get_hash() -> u64 {
    CURRENT_SCHEMA_HASH
}

extern "C" fn shim_get_state_version() -> u32 {
    CURRENT_STATE_VERSION
}

#[no_mangle]
pub extern "C" fn _create_game() -> PluginApi {
    let state = Box::into_raw(Box::new(MyGame::default())) as *mut c_void;

    PluginApi {
        state,
        on_load: shim_on_load,
        on_update: shim_on_update,
        on_unload: shim_on_unload,
        get_state_len: shim_get_state_len,
        save_state: shim_save_state,
        load_state: shim_load_state,
        drop_state: shim_drop_state,
        get_schema_hash: shim_get_hash,
        get_state_version: shim_get_state_version,
    }
}

#[cfg(test)]
mod safety_tests {
    use super::*;
    use engine_shared::plugin_api::{CURRENT_SCHEMA_HASH, CURRENT_STATE_VERSION};

    #[test]
    fn test_layout_change_requires_version_ack() {
        let game = MyGame::default();
        let current_size =
            bincode::serialized_size(&game).expect("Serialization of MyGame must succeed");

        const EXPECTED_SIZE: u64 = 8;
        const EXPECTED_VERSION: u32 = 1;
        const EXPECTED_HASH: u64 = 0x0123_4567_89AB_CDEF;

        assert_eq!(
            current_size, EXPECTED_SIZE,
            "STRUCT LAYOUT CHANGED! MyGame serialized size changed from {} to {}. Action required: bump CURRENT_STATE_VERSION and update EXPECTED_SIZE/EXPECTED_VERSION.",
            EXPECTED_SIZE, current_size
        );

        assert_eq!(
            CURRENT_STATE_VERSION, EXPECTED_VERSION,
            "VERSION MISMATCH! CURRENT_STATE_VERSION is {}, but EXPECTED_VERSION is {}. Update EXPECTED_VERSION when you intentionally bump the state version.",
            CURRENT_STATE_VERSION, EXPECTED_VERSION
        );

        assert_eq!(
            CURRENT_SCHEMA_HASH, EXPECTED_HASH,
            "HASH MISMATCH! CURRENT_SCHEMA_HASH is 0x{:X}, but EXPECTED_HASH is 0x{:X}. Update EXPECTED_HASH when you intentionally change the schema hash.",
            CURRENT_SCHEMA_HASH, EXPECTED_HASH
        );
    }
}