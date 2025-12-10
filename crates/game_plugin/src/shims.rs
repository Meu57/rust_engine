// crates/game_plugin/src/shims.rs

use std::ffi::c_void;
use std::io::Cursor;

use bincode;
use engine_ecs::World;
use engine_shared::{
    input_types::InputState,
    plugin_api::{
        FFIResult, FFIBuffer, HostContext, HostInterface, StateEnvelope,
        SNAPSHOT_MAGIC_HEADER, CURRENT_SCHEMA_HASH, CURRENT_STATE_VERSION,
    },
};

use crate::state::{MyGame, setup_scene};
use crate::systems;

fn catch_ffi_panic<F>(f: F) -> FFIResult
where
    F: FnOnce() -> FFIResult + std::panic::UnwindSafe,
{
    match std::panic::catch_unwind(f) {
        Ok(res) => res,
        Err(_) => FFIResult::PanicDetected,
    }
}

pub extern "C" fn on_load(
    state: *mut c_void,
    ctx: *mut HostContext,
    iface: *const HostInterface,
) -> FFIResult {
    catch_ffi_panic(|| {
        if state.is_null() || iface.is_null() {
            return FFIResult::Error;
        }

        unsafe {
            let game = &mut *(state as *mut MyGame);
            let host = &*iface;
            let world = &mut *(ctx as *mut World);

            game.bind_host_resources(host);

            if !game.scene_initialized {
                setup_scene(world);
                game.scene_initialized = true;
            }
        }

        FFIResult::Success
    })
}

pub extern "C" fn on_update(
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
            systems::camera::update_camera(world, dt);

            if let Some(spawn_fn) = game.spawn_fn {
                let ctx_ptr = world as *mut World as *mut HostContext;
                systems::enemy::spawn_enemies(spawn_fn, ctx_ptr, &mut game.spawn_timer, dt);
            }
        }

        FFIResult::Success
    })
}

pub extern "C" fn on_unload(_state: *mut c_void, _ctx: *mut HostContext) -> FFIResult {
    FFIResult::Success
}

pub extern "C" fn get_state_len(state: *mut c_void) -> usize {
    if state.is_null() {
        return 0;
    }
    let game = unsafe { &*(state as *mut MyGame) };
    let payload = bincode::serialized_size(game).unwrap_or(0) as usize;
    std::mem::size_of::<StateEnvelope>() + payload
}

pub extern "C" fn save_state(state: *mut c_void, buf: FFIBuffer) -> FFIResult {
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
            std::ptr::copy_nonoverlapping(
                &envelope as *const _ as *const u8,
                buf.ptr,
                header_len,
            );

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

pub extern "C" fn load_state(state: *mut c_void, buf: FFIBuffer) -> FFIResult {
    catch_ffi_panic(|| {
        if state.is_null() || buf.ptr.is_null() {
            return FFIResult::Error;
        }

        let game = unsafe { &mut *(state as *mut MyGame) };
        let header_len = std::mem::size_of::<StateEnvelope>();

        if buf.len < header_len {
            return FFIResult::Error;
        }

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
                    // Prevent double-spawning on hot-reload
                    game.scene_initialized = true; 
                    FFIResult::Success
                }
                Err(_) => FFIResult::Error,
            }
        }
    })
}

pub extern "C" fn drop_state(state: *mut c_void) {
    if !state.is_null() {
        unsafe { drop(Box::from_raw(state as *mut MyGame)); }
    }
}

pub extern "C" fn get_hash() -> u64 {
    CURRENT_SCHEMA_HASH
}

pub extern "C" fn get_state_version() -> u32 {
    CURRENT_STATE_VERSION
}