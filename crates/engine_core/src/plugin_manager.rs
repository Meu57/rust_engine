// crates/engine_core/src/plugin_manager.rs

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use libloading::{Library, Symbol};

use engine_ecs::World;
use engine_shared::input_types::InputState;
use engine_shared::plugin_api::{
    FFIResult,
    FFIBuffer,
    HostContext,
    HostInterface,
    PluginApi,
    StateEnvelope,
    SNAPSHOT_MAGIC_HEADER,
};

pub struct PluginHandle {
    pub api: PluginApi,
    pub lib: Library,
    pub path: PathBuf,
}

pub enum PluginRuntimeState {
    Running,
    PausedError(String),
}

pub struct PluginManager {
    pub plugin: PluginHandle,
    pub runtime_state: PluginRuntimeState,
    plugin_source_path: PathBuf,
    last_reload: Option<Instant>,
    reload_debounce: Duration,
}

impl PluginManager {
    pub fn new(path: &str) -> Self {
        let source_path = Path::new(path).to_path_buf();
        let plugin = unsafe { load_plugin(&source_path).expect("Failed to load initial plugin") };

        Self {
            plugin,
            runtime_state: PluginRuntimeState::Running,
            plugin_source_path: source_path,
            last_reload: None,
            reload_debounce: Duration::from_millis(500),
        }
    }

    pub fn initial_load(&self, world: &mut World, host_interface: &HostInterface) {
        unsafe {
            let res = (self.plugin.api.on_load)(
                self.plugin.api.state,
                world as *mut _ as *mut HostContext,
                host_interface as *const HostInterface,
            );
            if res != FFIResult::Success {
                eprintln!("⚠️ Warning: Plugin initial load returned {:?}", res);
            }
        }
    }

    pub fn update(&mut self, world: &mut World, input: &InputState, dt: f32) {
        if matches!(self.runtime_state, PluginRuntimeState::PausedError(_)) {
            return;
        }

        let res = unsafe {
            (self.plugin.api.on_update)(
                self.plugin.api.state,
                world as *mut _ as *mut HostContext,
                input as *const InputState,
                dt,
            )
        };

        match res {
            FFIResult::Success => {}
            FFIResult::PanicDetected => {
                eprintln!("❌ Plugin PANIC during update. Entering PausedError.");
                self.runtime_state =
                    PluginRuntimeState::PausedError("Panic during update".into());
            }
            other => {
                // Non-success from update is unusual but we log and keep running.
                eprintln!("⚠️ Plugin on_update returned {:?}", other);
            }
        }
    }

    fn save_plugin_state(&mut self) -> Option<Vec<u8>> {
        let mut retry_count = 0;
        let max_retries = 3;

        loop {
            if retry_count >= max_retries {
                eprintln!(
                    "⚠️ Aborting save after {} retries (state growing too fast).",
                    retry_count
                );
                return None;
            }

            let required_len = (self.plugin.api.get_state_len)(self.plugin.api.state);
            if required_len == 0 {
                return Some(Vec::new());
            }

            let mut buffer = vec![0u8; required_len];
            let ffi_buffer = FFIBuffer {
                ptr: buffer.as_mut_ptr(),
                len: buffer.len(),
            };

            let result = (self.plugin.api.save_state)(self.plugin.api.state, ffi_buffer);

            match result {
                FFIResult::Success => return Some(buffer),
                FFIResult::BufferTooSmall => {
                    retry_count += 1;
                    continue;
                }
                FFIResult::PanicDetected => {
                    eprintln!("❌ Plugin PANIC during save! State may be corrupt.");
                    self.runtime_state =
                        PluginRuntimeState::PausedError("Panic during save".into());
                    return None;
                }
                other => {
                    eprintln!("⚠️ save_state failed: {:?}", other);
                    return None;
                }
            }
        }
    }

    pub fn try_hot_reload(
        &mut self,
        world: &mut World,
        host_interface: &HostInterface,
    ) -> bool {
        let now = Instant::now();
        if let Some(last) = self.last_reload {
            if now.duration_since(last) < self.reload_debounce {
                return false;
            }
        }
        self.last_reload = Some(now);

        println!("🔄 Hot Reload requested...");

        // 1. SAVE STATE
        let snapshot = if matches!(self.runtime_state, PluginRuntimeState::Running) {
            self.save_plugin_state()
        } else {
            None
        };

        // 2. UNLOAD OLD
        unsafe {
            (self.plugin.api.drop_state)(self.plugin.api.state);
        }
        // dropping PluginHandle when we overwrite self.plugin
        let old_path = self.plugin.path.clone();
        // Clean up temp file
        let _ = fs::remove_file(&old_path);

        // 3. LOAD NEW
        let new_plugin = match unsafe { load_plugin(&self.plugin_source_path) } {
            Ok(p) => p,
            Err(e) => {
                eprintln!("❌ Failed to load new plugin: {e}");
                self.runtime_state =
                    PluginRuntimeState::PausedError(format!("Failed to load new plugin: {e}"));
                return false;
            }
        };
        self.plugin = new_plugin;

        // 4. RESTORE STATE
        if let Some(mut bytes) = snapshot {
            if !bytes.is_empty() && bytes.len() >= std::mem::size_of::<StateEnvelope>() {
                let header_size = std::mem::size_of::<StateEnvelope>();
                let mut envelope = StateEnvelope {
                    magic_header: 0,
                    state_version: 0,
                    schema_hash: 0,
                    payload_len: 0,
                };

                unsafe {
                    std::ptr::copy_nonoverlapping(
                        bytes.as_ptr(),
                        &mut envelope as *mut StateEnvelope as *mut u8,
                        header_size,
                    );
                }

                if envelope.magic_header == SNAPSHOT_MAGIC_HEADER {
                    let ffi_buffer = FFIBuffer {
                        ptr: bytes.as_mut_ptr(),
                        len: bytes.len(),
                    };
                    let res = (self.plugin.api.load_state)(self.plugin.api.state, ffi_buffer);

                    match res {
                        FFIResult::Success => {
                            println!("✅ State restored successfully.");
                        }
                        FFIResult::SchemaMismatch => {
                            eprintln!(
                                "⚠️ Schema mismatch during load_state. Using default state."
                            );
                        }
                        FFIResult::PanicDetected => {
                            eprintln!(
                                "❌ Plugin PANIC during load_state. Entering PausedError."
                            );
                            self.runtime_state = PluginRuntimeState::PausedError(
                                "Panic during load_state".into(),
                            );
                            return false;
                        }
                        other => {
                            eprintln!(
                                "⚠️ load_state failed ({:?}). Using default state.",
                                other
                            );
                        }
                    }
                }
            }
        }

        // 5. REBIND HOST RESOURCES
        unsafe {
            let res = (self.plugin.api.on_load)(
                self.plugin.api.state,
                world as *mut _ as *mut HostContext,
                host_interface as *const HostInterface,
            );
            if res != FFIResult::Success {
                eprintln!("⚠️ on_load failed after reload ({:?})", res);
            }
        }

        self.runtime_state = PluginRuntimeState::Running;
        true
    }
}

unsafe fn load_plugin(path: &Path) -> Result<PluginHandle, Box<dyn std::error::Error>> {
    let copy_path = unique_copy_path(path)?;
    fs::copy(path, &copy_path)?;

    let lib = Library::new(&copy_path)?;
    let create_fn: Symbol<extern "C" fn() -> PluginApi> = lib.get(b"_create_game")?;
    let api = create_fn();

    Ok(PluginHandle {
        api,
        lib,
        path: copy_path,
    })
}

fn unique_copy_path(original: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let ts = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    let stem = original
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or("plugin");
    let ext = original.extension().and_then(OsStr::to_str).unwrap_or("dll");
    Ok(original.with_file_name(format!("{stem}_loaded_{ts}.{ext}")))
}
