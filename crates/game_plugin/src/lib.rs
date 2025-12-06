// crates/game_plugin/src/lib.rs

mod systems;

use std::ffi::c_void;
use std::io::Cursor;

use engine_ecs::World;
use engine_shared::{
    ActionId,
    HostContext,
    HostInterface,
    InputState,
    PluginApi,
    SnapshotHeader,
    ACTION_NOT_FOUND,
    ENGINE_API_VERSION,
    calculate_layout_hash,
};
use serde::{Deserialize, Serialize};
use bincode;

// -----------------------------------------------------------------------------
// Snapshot framing
// -----------------------------------------------------------------------------

/// Magic number to verify this is our engine's snapshot.
const SNAPSHOT_MAGIC: u64 = 0xDEAD_BEEF_CAFE_BABE;

/// Logical layout ID for MyGame snapshot.
/// Change this string when you do breaking changes to MyGame.
const MYGAME_LAYOUT_ID: &str = "MyGame_v1";

// -----------------------------------------------------------------------------
// Concrete plugin instance
// -----------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
pub struct MyGame {
    pub spawn_timer: f32,

    // #[serde(default)] allows adding new fields safely.
    // If the old snapshot lacks this field, it uses Default::default().
    #[serde(default)]
    pub score: u32,

    // We do NOT serialize these, they are re-acquired on load.
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
    /// Internal load hook called by FFI shim.
    ///
    /// `snapshot` is a raw byte blob written by `shim_save_state`:
    /// [SnapshotHeader][bincode(MyGame)]
    fn do_on_load(
        &mut self,
        _world: &mut World,
        host: &HostInterface,
        snapshot: Option<&[u8]>,
    ) -> bool {
        // 1. Re-acquire function pointers and IDs (these are transient).
        self.actions[0] = (host.get_action_id)(b"MoveUp".as_ptr(), b"MoveUp".len());
        self.actions[1] = (host.get_action_id)(b"MoveDown".as_ptr(), b"MoveDown".len());
        self.actions[2] = (host.get_action_id)(b"MoveLeft".as_ptr(), b"MoveLeft".len());
        self.actions[3] = (host.get_action_id)(b"MoveRight".as_ptr(), b"MoveRight".len());
        self.spawn_fn = Some(host.spawn_enemy);

        // 2. Deserialize state if provided.
        if let Some(bytes) = snapshot {
            let header_size = core::mem::size_of::<SnapshotHeader>();

            if bytes.len() < header_size {
                eprintln!("[Plugin] Snapshot too small for header!");
                return false;
            }

            // SAFETY: header may not be aligned; we must copy into a local.
            let mut header = SnapshotHeader {
                magic: 0,
                version: 0,
                layout_hash: 0,
                data_len: 0,
            };

            unsafe {
                core::ptr::copy_nonoverlapping(
                    bytes.as_ptr(),
                    &mut header as *mut SnapshotHeader as *mut u8,
                    header_size,
                );
            }

            let payload = &bytes[header_size..];

            if header.magic != SNAPSHOT_MAGIC {
                eprintln!("[Plugin] Invalid snapshot magic!");
                return false;
            }

            if header.data_len != payload.len() {
                eprintln!(
                    "[Plugin] Snapshot data_len mismatch (header: {}, payload: {})",
                    header.data_len,
                    payload.len()
                );
                return false;
            }

            let expected_hash = calculate_layout_hash(MYGAME_LAYOUT_ID);
            if header.layout_hash != expected_hash {
                eprintln!(
                    "[Plugin] Snapshot layout hash mismatch, resetting to default. expected={}, got={}",
                    expected_hash,
                    header.layout_hash
                );
                *self = MyGame::default();
                // re-bind host resources
                return self.do_on_load(_world, host, None);
            }

            match bincode::deserialize::<MyGame>(payload) {
                Ok(loaded_state) => {
                    // Copy serialized fields; transient ones already rebound.
                    self.spawn_timer = loaded_state.spawn_timer;
                    self.score = loaded_state.score;

                    println!(
                        "[Plugin] State Restored! Timer: {}, Score: {}",
                        self.spawn_timer, self.score
                    );
                    true
                }
                Err(e) => {
                    eprintln!("[Plugin] Deserialization failed: {e}");

                    // Fallback to default state on failure and re-bind host resources.
                    *self = MyGame::default();
                    self.do_on_load(_world, host, None)
                }
            }
        } else {
            // No previous snapshot; just start with default / current state.
            true
        }
    }

    fn do_update(&mut self, world: &mut World, input: &InputState, dt: f32) {
        systems::player::update_player(world, input, dt, &self.actions);

        if let Some(spawn_fn) = self.spawn_fn {
            let world_ptr = world as *mut World as *mut HostContext;
            systems::enemy::spawn_enemies(spawn_fn, world_ptr, &mut self.spawn_timer, dt);
        }
    }
}

// -----------------------------------------------------------------------------
// FFI SHIMS (The Firewall)
// -----------------------------------------------------------------------------

extern "C" fn shim_on_load(
    state: *mut c_void,
    ctx: *mut HostContext,
    host: *const HostInterface,
    snap_ptr: *const u8,
    snap_len: usize,
) -> bool {
    if state.is_null() || ctx.is_null() || host.is_null() {
        eprintln!("[Plugin] shim_on_load called with null state/ctx/host");
        return false;
    }

    unsafe {
        let game = &mut *(state as *mut MyGame);
        let world = &mut *(ctx as *mut World);
        let host = &*host;

        let snapshot = if !snap_ptr.is_null() && snap_len > 0 {
            Some(core::slice::from_raw_parts(snap_ptr, snap_len))
        } else {
            None
        };

        game.do_on_load(world, host, snapshot)
    }
}

/// Strategy B: Host queries size first.
extern "C" fn shim_get_state_size(state: *mut c_void) -> usize {
    if state.is_null() {
        return 0;
    }

    unsafe {
        let game = &*(state as *mut MyGame);
        let header_size = core::mem::size_of::<SnapshotHeader>();
        let payload_size = bincode::serialized_size(game).unwrap_or(0) as usize;

        header_size + payload_size
    }
}

/// Strategy B: Host allocates, Plugin writes.
extern "C" fn shim_save_state(state: *mut c_void, buffer: *mut u8, len: usize) -> usize {
    if state.is_null() || buffer.is_null() {
        return 0;
    }

    unsafe {
        let game = &*(state as *mut MyGame);
        let header_size = core::mem::size_of::<SnapshotHeader>();
        let payload_size = bincode::serialized_size(game).unwrap_or(0) as usize;
        let total_size = header_size + payload_size;

        if len < total_size {
            eprintln!(
                "[Plugin] shim_save_state: buffer too small (have {}, need {})",
                len, total_size
            );
            return 0;
        }

        // Build header.
        let header = SnapshotHeader {
            magic: SNAPSHOT_MAGIC,
            version: 1, // bump on breaking changes to MyGame
            layout_hash: calculate_layout_hash(MYGAME_LAYOUT_ID),
            data_len: payload_size,
        };

        // SAFETY: buffer may not be aligned for SnapshotHeader; write as bytes.
        core::ptr::copy_nonoverlapping(
            &header as *const SnapshotHeader as *const u8,
            buffer,
            header_size,
        );

        // Write payload immediately after header.
        let payload_ptr = buffer.add(header_size);
        let payload_slice = core::slice::from_raw_parts_mut(payload_ptr, payload_size);
        let mut cursor = Cursor::new(payload_slice);

        if let Err(e) = bincode::serialize_into(&mut cursor, game) {
            eprintln!("[Plugin] shim_save_state: serialize_into failed: {e}");
            return 0;
        }

        total_size
    }
}

extern "C" fn shim_update(
    state: *mut c_void,
    ctx: *mut HostContext,
    input: *const InputState,
    dt: f32,
) {
    if state.is_null() || ctx.is_null() || input.is_null() {
        return;
    }

    unsafe {
        let game = &mut *(state as *mut MyGame);
        let world = &mut *(ctx as *mut World);
        let input = &*input;
        game.do_update(world, input, dt);
    }
}

extern "C" fn shim_on_unload(_state: *mut c_void, _ctx: *mut HostContext) {
    // Optional; no special unload behavior yet.
}

extern "C" fn shim_drop(state: *mut c_void) {
    if state.is_null() {
        return;
    }

    unsafe {
        // Free plugin-owned heap memory safely inside the plugin.
        let _boxed: Box<MyGame> = Box::from_raw(state as *mut MyGame);
        // dropped here
    }
}

extern "C" fn shim_get_hash() -> u64 {
    // Return the hash of the snapshot layout we expect.
    calculate_layout_hash(MYGAME_LAYOUT_ID)
}

// -----------------------------------------------------------------------------
// EXPORTS
// -----------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn _create_game() -> PluginApi {
    let state = Box::into_raw(Box::new(MyGame::default())) as *mut c_void;

    PluginApi {
        state,
        on_load: shim_on_load,
        update: shim_update,
        on_unload: shim_on_unload,
        get_state_size: shim_get_state_size,
        save_state: shim_save_state,
        drop: shim_drop,
        get_layout_hash: shim_get_hash,
    }
}

#[no_mangle]
pub extern "C" fn get_api_version() -> u32 {
    ENGINE_API_VERSION
}