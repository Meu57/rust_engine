// crates/engine_core/src/plugin_manager.rs
use std::time::{Duration, Instant};
use crate::hot_reload::GamePlugin;
use engine_ecs::World;
use engine_shared::{HostContext, HostInterface, InputState};

pub struct PluginManager {
    plugin: GamePlugin,
    plugin_path: String,
    last_reload: Option<Instant>,
    reload_debounce: Duration,
}

impl PluginManager {
    pub fn new(path: &str) -> Self {
        let plugin = unsafe { GamePlugin::load(path).expect("Failed to load plugin") };
        Self {
            plugin,
            plugin_path: path.to_string(),
            last_reload: None,
            reload_debounce: Duration::from_millis(500),
        }
    }

    pub fn initial_load(&self, world: &mut World, host_interface: &HostInterface) {
        unsafe {
            if !(self.plugin.api.on_load)(
                self.plugin.api.state,
                world as *mut _ as *mut HostContext,
                host_interface as *const _,
                std::ptr::null(),
                0,
            ) {
                eprintln!("⚠️ Warning: Plugin initial load failed.");
            }
        }
    }

    pub fn update(&self, world: &mut World, input: &InputState, dt: f32) {
        unsafe {
            (self.plugin.api.update)(
                self.plugin.api.state,
                world as *mut _ as *mut HostContext,
                input as *const _,
                dt,
            );
        }
    }

    pub fn try_hot_reload(&mut self, world: &mut World, host_interface: &HostInterface) -> bool {
        let now = Instant::now();
        if let Some(last) = self.last_reload {
            if now.duration_since(last) < self.reload_debounce {
                return false;
            }
        }
        self.last_reload = Some(now);

        println!("🔄 Hot Reload requested...");

        // 1. SAVE STATE
        let required_size = (self.plugin.api.get_state_size)(self.plugin.api.state);

        let mut snapshot_buffer = if required_size > 0 {
            vec![0u8; required_size]
        } else {
            Vec::new()
        };

        let written_len = if !snapshot_buffer.is_empty() {
            (self.plugin.api.save_state)(
                self.plugin.api.state,
                snapshot_buffer.as_mut_ptr(),
                snapshot_buffer.len(),
            )
        } else {
            0
        };

        // SAFETY / ROBUSTNESS FIX
        if written_len == 0 && required_size > 0 {
            eprintln!("⚠️ Plugin failed to write snapshot. Reloading with fresh state.");
            snapshot_buffer.clear();
        } else if written_len < snapshot_buffer.len() {
            snapshot_buffer.truncate(written_len);
        }

        // 2. LOAD NEW PLUGIN
        match unsafe { GamePlugin::load(&self.plugin_path) } {
            Ok(mut new_plugin) => {
                println!("Verifying new plugin...");
                
                let snapshot_ptr = if !snapshot_buffer.is_empty() { 
                    snapshot_buffer.as_ptr() 
                } else { 
                    std::ptr::null() 
                };

                // 3. HANDSHAKE & RESTORE
                let success = unsafe {
                    (new_plugin.api.on_load)(
                        new_plugin.api.state,
                        world as *mut _ as *mut HostContext,
                        host_interface as *const _,
                        snapshot_ptr,
                        snapshot_buffer.len(),
                    )
                };

                if success {
                    self.plugin = new_plugin;
                    println!("✅ Hot Reload Success! State preserved.");
                    return true;
                } else {
                    eprintln!("❌ New plugin rejected the snapshot. Keeping old plugin.");
                }
            }
            Err(e) => {
                eprintln!("❌ Hot Reload Failed (Load Error): {e}");
            }
        }

        false
    }
}