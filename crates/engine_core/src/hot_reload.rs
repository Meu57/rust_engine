// crates/engine_core/src/hot_reload.rs

use libloading::{Library, Symbol};

use std::error::Error;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use engine_shared::{PluginApi, ENGINE_API_VERSION, calculate_layout_hash};

/// Must match the plugin's MYGAME_LAYOUT_ID
/// (see crates/game_plugin/src/lib.rs)
const MYGAME_LAYOUT_ID: &str = "MyGame_v1";

pub struct GamePlugin {
    pub api: PluginApi,
    _lib: Library,        // keep library alive for the lifetime of the plugin
    pub plugin_path: PathBuf,
}

impl GamePlugin {
    pub unsafe fn load(path: &str) -> Result<Self, Box<dyn Error>> {
        let original = Path::new(path);
        if !original.exists() {
            return Err(format!("Plugin file not found: {}", path).into());
        }

        // 1. Copy to a unique path to avoid file locking issues on Windows
        let copied_path = unique_copy_path(original)?;

        fs::copy(original, &copied_path).map_err(|e| {
            format!("Failed to copy plugin to temporary path: {}", e)
        })?;

        // 2. Load the library
        let lib = Library::new(&copied_path).map_err(|e| {
            format!("Failed to load library at {:?}: {}", copied_path, e)
        })?;

        // 3. Version Handshake
        type VersionFn = unsafe extern "C" fn() -> u32;
        let version_func: Symbol<VersionFn> = match lib.get(b"get_api_version") {
            Ok(sym) => sym,
            Err(e) => {
                return Err(format!(
                    "Plugin missing 'get_api_version' export: {}",
                    e
                )
                .into())
            }
        };

        let plugin_version = version_func();
        if plugin_version != ENGINE_API_VERSION {
            return Err(format!(
                "Plugin Version Mismatch! Engine: {}, Plugin: {}",
                ENGINE_API_VERSION, plugin_version
            )
            .into());
        }

        // 4. Load the Plugin API
        type CreateFn = unsafe extern "C" fn() -> PluginApi;
        let create_func: Symbol<CreateFn> = match lib.get(b"_create_game") {
            Ok(sym) => sym,
            Err(e) => {
                return Err(format!(
                    "Plugin missing '_create_game' export: {}",
                    e
                )
                .into())
            }
        };

        let api = create_func();

        // 5. STRUCTURAL HASH HANDSHAKE (Snapshot Layout)
        //
        // Host and Plugin must agree on the MyGame snapshot layout ID.
        let host_hash = calculate_layout_hash(MYGAME_LAYOUT_ID);
        let plugin_hash = (api.get_layout_hash)();

        if host_hash != plugin_hash {
            return Err(format!(
                "CRITICAL: Layout Mismatch! Host expects hash 0x{:X}, Plugin provided 0x{:X}. \
                 This means the MyGame snapshot layout differs. Recompile engine + plugin.",
                host_hash, plugin_hash
            ).into());
        }

        Ok(Self {
            api,
            _lib: lib,
            plugin_path: copied_path,
        })
    }
}

/// Helper to generate a unique filename (e.g., "game_plugin_loaded_1678822.dll")
fn unique_copy_path(original: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis();

    let stem = original
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or("game_plugin");

    let ext = original
        .extension()
        .and_then(OsStr::to_str)
        .unwrap_or("dll");

    let new_name = format!("{}_loaded_{}.{}", stem, ts, ext);
    Ok(original.with_file_name(new_name))
}

// Clean up the temporary DLL file when the plugin is unloaded
impl Drop for GamePlugin {
    fn drop(&mut self) {
        // Important: Let the plugin free its own state memory before we unload the lib
        (self.api.drop)(self.api.state);

        // Attempt to delete the temporary file.
        let _ = fs::remove_file(&self.plugin_path);
    }
}
