use engine_shared::GameLogic;
use libloading::{Library, Symbol};
use std::path::Path;
use std::fs;

pub struct GamePlugin {
    // We hold the Library to ensure the memory remains valid.
    // Order matters: 'api' must be dropped before '_lib'.
    pub api: Box<dyn GameLogic>,
    _lib: Library, 
}

impl GamePlugin {
    pub unsafe fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        // 1. Define paths
        let original_path = Path::new(path);
        let loaded_path = original_path.with_file_name("game_plugin_loaded.dll");

        // 2. Copy the DLL to avoid locking the original (Windows fix)
        fs::copy(original_path, &loaded_path)?;

        // 3. Load the COPIED library
        let lib = Library::new(&loaded_path)?;

        // 4. Find the entry point symbol
        // The signature must match the extern "C" function in game_plugin/lib.rs
        type CreateGameFn = unsafe extern "C" fn() -> *mut dyn GameLogic;
        
        let func: Symbol<CreateGameFn> = lib.get(b"_create_game")?;

        // 5. Call the factory function
        let raw_ptr = func();
        let mut api = Box::from_raw(raw_ptr);

        // 6. Initialize the plugin (optional hook)
        // We pass a dummy World pointer for now, effectively verifying ABI
        // (In a real scenario, we'd pass the actual world here if needed)
        // api.on_load(...); 

        tracing::info!("Loaded Game Plugin from: {:?}", loaded_path);

        Ok(Self {
            _lib: lib,
            api,
        })
    }
}