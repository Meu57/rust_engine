// crates/engine_core/src/app.rs

use std::sync::Mutex;

use crate::gui::GuiSystem;
use crate::input::{self, ActionRegistry, Arbiter, InputMap};
use crate::input::config::InputDefaults;
use crate::platform_runner::PlatformRunner;
use engine_shared::input_types::{ActionId, InputState};
use winit::keyboard::KeyCode;

/// High-level engine state container.
/// Does NOT own the OS loop, timing, or raw input.
/// Those are delegated to PlatformRunner / EngineLoop / InputPoller.
pub struct App {
    // Exposed within crate so PlatformRunner can orchestrate.
    pub(crate) registry: ActionRegistry,
    pub(crate) input_map: InputMap,
    pub(crate) arbiter: Arbiter,
    pub(crate) window_title: String,
    pub(crate) gui: GuiSystem,

    pub(crate) engine_toggle_inspector: ActionId,
    pub(crate) engine_request_hot_reload: ActionId,

    pub(crate) last_input_state: InputState,

    pub(crate) plugin_path: String,
}

impl App {
    /// Create a new App with a configurable plugin path.
    pub fn new(plugin_path: &str) -> Self {
        let mut registry = ActionRegistry::default();
        let mut input_map = InputMap::default();

        // 1. Delegate canonical movement bindings to InputDefaults.
        InputDefaults::setup(&mut registry, &mut input_map);

        // 2. Register engine-level actions as first-class actions.
        let engine_toggle_inspector = registry.register("Engine.ToggleInspector");
        let engine_request_hot_reload = registry.register("Engine.RequestHotReload");

        // Bind F1/F5 to these actions (no hard-coded branches in the loop).
        input_map.bind_logical(KeyCode::F1, engine_toggle_inspector);
        input_map.bind_logical(KeyCode::F5, engine_request_hot_reload);

        // 3. Publish registry globally for tools / plugins.
        let _ = input::GLOBAL_REGISTRY.set(Mutex::new(registry.clone()));

        // 4. Configure Arbiter layers from centralized defaults.
        let arbiter = Arbiter::new(InputDefaults::default_arbiter_layers(), 0.1);

        Self {
            registry,
            input_map,
            arbiter,
            window_title: "Rust Engine: Modular Architecture".to_string(),
            gui: GuiSystem::new(),

            engine_toggle_inspector,
            engine_request_hot_reload,

            last_input_state: InputState::default(),
            plugin_path: plugin_path.to_string(),
        }
    }

    /// Delegation: hand ownership to PlatformRunner, which drives the OS loop.
    pub fn run(self) {
        PlatformRunner::new(self).start();
    }
}
