// crates/engine_core/src/app.rs
use std::sync::Mutex;

use glam::Vec2;
use winit::{
    event::{ElementState, Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};

// Modules
use crate::renderer::Renderer;
use crate::input::{self, ActionRegistry, Arbiter, InputMap};
use crate::inspector;
use crate::scene;
use crate::host;
use crate::gui::GuiSystem;
use crate::plugin_manager::PluginManager;

use engine_shared::{PriorityLayer, ActionSignal, MovementSignal};
use engine_ecs::World;

pub struct App {
    registry: ActionRegistry,
    input_map: InputMap,
    arbiter: Arbiter,
    window_title: String,
    
    // Sub-systems
    gui: GuiSystem,
}

impl App {
    pub fn new() -> Self {
        let mut registry = ActionRegistry::default();
        let mut input_map = InputMap::default();

        // Register canonical actions
        let move_up = registry.register("MoveUp");
        let move_down = registry.register("MoveDown");
        let move_left = registry.register("MoveLeft");
        let move_right = registry.register("MoveRight");

        input_map.bind_logical(KeyCode::KeyW, move_up);
        input_map.bind_logical(KeyCode::KeyS, move_down);
        input_map.bind_logical(KeyCode::KeyA, move_left);
        input_map.bind_logical(KeyCode::KeyD, move_right);

        let _ = input::GLOBAL_REGISTRY.set(Mutex::new(registry.clone()));

        Self {
            registry,
            input_map,
            arbiter: Arbiter::default(),
            window_title: "Rust Engine: Modular Architecture".to_string(),
            gui: GuiSystem::new(),
        }
    }

    pub fn run(mut self) {
        let event_loop = EventLoop::new().unwrap();
        let window = WindowBuilder::new()
            .with_title(&self.window_title)
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0))
            .build(&event_loop)
            .unwrap();

        // Initialize GUI with the window
        self.gui.init(&window);

        let mut renderer = pollster::block_on(Renderer::new(&window));
        let mut world = World::new();

        scene::setup_default_world(&mut world);
        let host_interface = host::create_interface();

        // Initialize Plugin Manager
        let mut plugin_manager = PluginManager::new("target/debug/game_plugin.dll");
        plugin_manager.initial_load(&mut world, &host_interface);

        let mut active_keys: Vec<KeyCode> = Vec::new();

        event_loop
            .run(move |event, elwt| {
                elwt.set_control_flow(ControlFlow::Poll);

                // 1. GUI Event Handling
                if let Event::WindowEvent { event: ref w_event, .. } = event {
                    self.gui.handle_event(&window, w_event);
                }

                match event {
                    Event::WindowEvent { event, .. } => match event {
                        WindowEvent::CloseRequested => elwt.exit(),

                        WindowEvent::KeyboardInput { event: key_event, .. } => {
                            if key_event.state == ElementState::Pressed {
                                if let PhysicalKey::Code(KeyCode::F1) = key_event.physical_key {
                                    self.gui.toggle_inspector();
                                }
                                
                                // Hot Reload Delegation
                                if let PhysicalKey::Code(KeyCode::F5) = key_event.physical_key {
                                    plugin_manager.try_hot_reload(&mut world, &host_interface);
                                }
                            }

                            if self.gui.wants_keyboard_input() {
                                return;
                            }

                            // Input Tracking
                            if let PhysicalKey::Code(keycode) = key_event.physical_key {
                                if key_event.state == ElementState::Pressed {
                                    if !active_keys.contains(&keycode) {
                                        active_keys.push(keycode);
                                    }
                                } else {
                                    active_keys.retain(|&k| k != keycode);
                                }
                            }
                        }

                        WindowEvent::Resized(size) => renderer.resize(size),

                        WindowEvent::RedrawRequested => {
                            // 2. GUI Drawing & Rendering
                            
                            // FIX: We copy 'show_inspector' to a local variable to break the borrow.
                            // The closure borrows 'inspector_open', unrelated to 'self.gui'.
                            let mut inspector_open = self.gui.show_inspector;
                            
                            let (primitives, textures_delta) = self.gui.draw(&window, |ctx| {
                                inspector::show(ctx, &self.arbiter, &mut inspector_open);
                            });

                            // Write the state back
                            self.gui.show_inspector = inspector_open;

                            let _ = renderer.render(
                                &world,
                                Some((&self.gui.ctx, &primitives, &textures_delta)),
                            );
                        }
                        _ => (),
                    },

                    Event::AboutToWait => {
                        let dt = 1.0 / 60.0;
                        self.arbiter.clear();

                        // 3. Input Processing
                        for &key in &active_keys {
                            let physical = PhysicalKey::Code(key);
                            if let Some(action_id) =
                                self.input_map.map_signal_to_intent(Some(key), physical)
                            {
                                self.arbiter.add_action(ActionSignal {
                                    layer: PriorityLayer::Control,
                                    action_id,
                                    active: true,
                                });
                            }
                        }

                        // Debug: Reflex test
                        if active_keys.contains(&KeyCode::KeyP) {
                            self.arbiter.add_movement(MovementSignal {
                                layer: PriorityLayer::Reflex,
                                vector: Vec2::ZERO,
                                weight: 1.0,
                            });
                            self.arbiter.add_action(ActionSignal {
                                layer: PriorityLayer::Reflex,
                                action_id: 0,
                                active: false,
                            });
                        }

                        let final_input_state = self.arbiter.resolve();

                        // 4. Game Update
                        plugin_manager.update(&mut world, &final_input_state, dt);

                        window.request_redraw();
                    }
                    _ => (),
                }
            })
            .unwrap();
    }
}