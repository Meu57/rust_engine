// crates/engine_core/src/hot_reload.rs
use libloading::{Library, Symbol};
use std::error::Error;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use engine_shared::{GameLogic, ENGINE_API_VERSION};

pub struct GamePlugin {
    pub api: Box<dyn GameLogic>,
    // We keep the library handle here. When GamePlugin is dropped, 
    // _lib is dropped, which unloads the DLL.
    _lib: Library, 
    pub plugin_path: PathBuf,
}

impl GamePlugin {
    pub unsafe fn load(path: &str) -> Result<Self, Box<dyn Error>> {
        let original = Path::new(path);
        if !original.exists() {
            return Err(format!("Plugin file not found: {}", path).into());
        }

        // 1. Copy to a unique path to avoid file locking issues on Windows
        //    If we just used "game_plugin_loaded.dll", the OS might lock it 
        //    preventing the NEXT reload from writing to it.
        let copied_path = unique_copy_path(original)?;
        
        fs::copy(original, &copied_path).map_err(|e| {
            format!("Failed to copy plugin to temporary path: {}", e)
        })?;

        // 2. Load the library
        let lib = Library::new(&copied_path).map_err(|e| {
            format!("Failed to load library at {:?}: {}", copied_path, e)
        })?;

        // 3. Version Handshake
        // We define the function signature type
        type VersionFn = unsafe extern "C" fn() -> u32;

        // We try to find the symbol. We use the type `VersionFn` directly.
        let version_func: Symbol<VersionFn> = match lib.get(b"get_api_version") {
            Ok(sym) => sym,
            Err(e) => {
                // Cleanup before returning error
                return Err(format!("Plugin missing 'get_api_version' export: {}", e).into());
            }
        };

        // Call the function
        let plugin_version = version_func();

        if plugin_version != ENGINE_API_VERSION {
            return Err(format!(
                "Plugin Version Mismatch! Engine: {}, Plugin: {}", 
                ENGINE_API_VERSION, plugin_version
            ).into());
        }

        // 4. Load the Game Logic
        type CreateFn = unsafe extern "C" fn() -> *mut dyn GameLogic;
        
        let create_func: Symbol<CreateFn> = match lib.get(b"_create_game") {
            Ok(sym) => sym,
            Err(e) => return Err(format!("Plugin missing '_create_game' export: {}", e).into()),
        };

        let raw_ptr = create_func();
        if raw_ptr.is_null() {
            return Err("Plugin returned a null pointer for GameLogic".into());
        }

        let api = Box::from_raw(raw_ptr);

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

    let stem = original.file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or("game_plugin");
    
    let ext = original.extension()
        .and_then(OsStr::to_str)
        .unwrap_or("dll");

    let new_name = format!("{}_loaded_{}.{}", stem, ts, ext);
    Ok(original.with_file_name(new_name))
}

// Optional: Implement Drop to clean up the temporary DLL file when the plugin is unloaded
impl Drop for GamePlugin {
    fn drop(&mut self) {
        // Attempt to delete the temporary file. 
        // On Windows, this might fail if the library is still technically "in use" 
        // by the OS for a few milliseconds, but it helps keep the temp folder clean.
        let _ = fs::remove_file(&self.plugin_path);
    }
}