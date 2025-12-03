// crates/engine_core/src/lib.rs
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use glam::Vec2;
use winit::{
    event::{Event, WindowEvent, ElementState},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
    keyboard::{KeyCode, PhysicalKey}, // Updated for winit 0.29
};

mod hot_reload;
mod renderer; 
use renderer::Renderer;

use engine_shared::{
    ActionId, ACTION_NOT_FOUND, HostInterface, InputState, MAX_AXES,
    CTransform, CSprite, CPlayer, CEnemy,
};

use engine_ecs::World; 

// Simple action registry
#[derive(Default, Clone)]
struct ActionRegistry {
    name_to_id: HashMap<String, ActionId>,
    next_id: ActionId,
}

impl ActionRegistry {
    fn register(&mut self, name: &str) -> ActionId {
        if let Some(&id) = self.name_to_id.get(name) { return id; }
        let id = self.next_id;
        self.name_to_id.insert(name.to_string(), id);
        self.next_id = self.next_id.wrapping_add(1);
        id
    }

    fn get_id(&self, name: &str) -> Option<ActionId> {
        self.name_to_id.get(name).copied()
    }
}

// Input mapping: from Physical Key Codes to ActionId (intent).
#[derive(Default)]
struct InputMap {
    key_bindings: HashMap<KeyCode, ActionId>,
}

impl InputMap {
    fn bind(&mut self, key: KeyCode, action: ActionId) {
        self.key_bindings.insert(key, action);
    }
    fn map_signal_to_intent(&self, key: KeyCode) -> Option<ActionId> {
        self.key_bindings.get(&key).copied()
    }
}

// For FFI: global registry snapshot exposed via HostInterface.
// Using OnceLock (Std Lib) instead of lazy_static to avoid extra dependencies.
static GLOBAL_REGISTRY: OnceLock<Mutex<ActionRegistry>> = OnceLock::new();

/// extern "C" implementation passed into plugins
extern "C" fn host_get_action_id(name_ptr: *const u8, name_len: usize) -> ActionId {
    unsafe {
        if name_ptr.is_null() || name_len == 0 { return ACTION_NOT_FOUND; }
        let slice = std::slice::from_raw_parts(name_ptr, name_len);
        if let Ok(name) = std::str::from_utf8(slice) {
            // Access the global singleton safely
            if let Some(mutex) = GLOBAL_REGISTRY.get() {
                if let Ok(reg) = mutex.lock() {
                    return reg.get_id(name).unwrap_or(ACTION_NOT_FOUND);
                }
            }
        }
    }
    ACTION_NOT_FOUND
}

pub struct App {
    registry: ActionRegistry,
    input_map: InputMap,
    window_title: String,
}

impl App {
    pub fn new() -> Self {
        let mut registry = ActionRegistry::default();
        let mut input_map = InputMap::default();

        // Host defines the canonical action set
        let move_up = registry.register("MoveUp");
        let move_down = registry.register("MoveDown");
        let move_left = registry.register("MoveLeft");
        let move_right = registry.register("MoveRight");

        // Logical key bindings (Updated to KeyCode for winit 0.29)
        input_map.bind(KeyCode::KeyW, move_up);
        input_map.bind(KeyCode::KeyS, move_down);
        input_map.bind(KeyCode::KeyA, move_left);
        input_map.bind(KeyCode::KeyD, move_right);

        // Publish snapshot for FFI query (Initialize OnceLock)
        let _ = GLOBAL_REGISTRY.set(Mutex::new(registry.clone()));

        Self { registry, input_map, window_title: "Rust Engine".to_string() }
    }

    pub fn run(self) {
        let event_loop = EventLoop::new().unwrap(); // unwrapped for 0.29
        let window = WindowBuilder::new()
            .with_title(&self.window_title)
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0))
            .build(&event_loop)
            .unwrap();

        let mut renderer = pollster::block_on(Renderer::new(&window));
        let mut world = World::new();

        // Register expected components
        world.register_component::<CTransform>();
        world.register_component::<CPlayer>();
        world.register_component::<CEnemy>();
        world.register_component::<CSprite>();

        // spawn player
        let player = world.spawn();
        world.add_component(player, CTransform { pos: Vec2::new(100.0,100.0), ..Default::default() });
        world.add_component(player, CPlayer);
        world.add_component(player, CSprite::default());

        // load plugin (hot-reload wrapper)
        let plugin_path = "target/debug/game_plugin.dll";
        let mut game_plugin = unsafe {
            hot_reload::GamePlugin::load(plugin_path).expect("Failed to load plugin")
        };

        // Build HostInterface
        let host_interface = HostInterface {
            get_action_id: host_get_action_id,
            log: None,
        };

        // call plugin negotiation
        game_plugin.api.on_load(&mut world, &host_interface);

        // current InputState (compact, arbiter result)
        let mut current_input = InputState::default();

        event_loop.run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Poll); // Updated for 0.29

            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => elwt.exit(),
                    
                    // UPDATED INPUT HANDLING FOR WINIT 0.29
                    WindowEvent::KeyboardInput { event: key_event, .. } => {
                        if let PhysicalKey::Code(keycode) = key_event.physical_key {
                            if let Some(action_id) = self.input_map.map_signal_to_intent(keycode) {
                                match key_event.state {
                                    ElementState::Pressed => current_input.digital_mask |= 1u64 << action_id,
                                    ElementState::Released => current_input.digital_mask &= !(1u64 << action_id),
                                }
                            }
                        }
                    }
                    
                    WindowEvent::Resized(size) => renderer.resize(size),
                    WindowEvent::RedrawRequested => { let _ = renderer.render(&world); }
                    _ => (),
                },
                // UPDATED EVENT LOOP HOOK FOR WINIT 0.29
                Event::AboutToWait => {
                    let dt = 1.0 / 60.0;
                    // pass copy of input to plugin update
                    game_plugin.api.update(&mut world, &current_input, dt);
                    window.request_redraw();
                }
                _ => (),
            }
        }).unwrap();
    }
}