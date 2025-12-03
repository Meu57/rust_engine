// crates/engine_core/src/hot_reload.rs
use libloading::{Library, Symbol};
use std::fs;
use std::path::Path;
use engine_shared::GameLogic;
use std::error::Error;
use std::ffi::c_void;

pub struct GamePlugin {
    pub api: Box<dyn GameLogic>,
    _lib: Library, // keep library handle alive while api is used
}

impl GamePlugin {
    /// Load a plugin DLL/shared-object. Copies the file first (Windows).
    /// Safety: plugin must export `#[no_mangle] pub extern "C" fn _create_game() -> *mut dyn GameLogic`
    pub unsafe fn load(path: &str) -> Result<Self, Box<dyn Error>> {
        let original = Path::new(path);
        if !original.exists() {
            return Err(format!("Plugin file not found: {}", path).into());
        }

        // copy to avoid locking original file while plugin runs (useful for hot-rebuild)
        let copied = original.with_file_name("game_plugin_loaded.dll");
        let _ = fs::copy(original, &copied)?;

        let lib = Library::new(&copied)?;
        type CreateFn = unsafe extern "C" fn() -> *mut dyn GameLogic;
        let func: Symbol<CreateFn> = lib.get(b"_create_game")?;
        let raw = func();
        if raw.is_null() {
            return Err("plugin returned null from _create_game".into());
        }
        let api = Box::from_raw(raw);

        Ok(Self { api, _lib: lib })
    }
}
