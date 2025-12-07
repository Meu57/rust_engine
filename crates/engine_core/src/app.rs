// crates/engine_core/src/app.rs

use std::sync::Mutex;

use glam::Vec2;
use winit::{
    event::{ElementState, Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};

use crate::renderer::Renderer;
use crate::input::{self, ActionRegistry, Arbiter, InputMap};
use crate::input::arbiter::{LayerConfig, MovementSignal, ActionSignal, channels};
use crate::inspector;
use crate::scene;
use crate::host;
use crate::gui::GuiSystem;
use crate::plugin_manager::{PluginManager, PluginRuntimeState};

use engine_shared::input_types::{PriorityLayer, canonical_actions};
use engine_ecs::World;

pub struct App {
    registry: ActionRegistry,
    input_map: InputMap,
    arbiter: Arbiter,
    window_title: String,
    gui: GuiSystem,
}

impl App {
    pub fn new() -> Self {
        let mut registry = ActionRegistry::default();
        let mut input_map = InputMap::default();

        // 1. Register canonical actions FIRST to guarantee IDs 0..3
        let move_up = registry.register("MoveUp");
        let move_down = registry.register("MoveDown");
        let move_left = registry.register("MoveLeft");
        let move_right = registry.register("MoveRight");

        // Verify alignment (optional panic if order is wrong)
        assert_eq!(move_up, canonical_actions::MOVE_UP);
        assert_eq!(move_right, canonical_actions::MOVE_RIGHT);

        input_map.bind_logical(KeyCode::KeyW, move_up);
        input_map.bind_logical(KeyCode::KeyS, move_down);
        input_map.bind_logical(KeyCode::KeyA, move_left);
        input_map.bind_logical(KeyCode::KeyD, move_right);

        let _ = input::GLOBAL_REGISTRY.set(Mutex::new(registry.clone()));

        // 2. Configure Arbiter
        let layers = vec![
            LayerConfig {
                layer: PriorityLayer::Reflex,
                allowed_mask_when_active: 0, // Block everything
                lock_on_activation: true,
                lock_frames_on_activation: 30,
            },
            LayerConfig {
                layer: PriorityLayer::Cutscene,
                allowed_mask_when_active: 0,
                lock_on_activation: false,
                lock_frames_on_activation: 0,
            },
            LayerConfig {
                layer: PriorityLayer::Control,
                allowed_mask_when_active: channels::MASK_ALL,
                lock_on_activation: false,
                lock_frames_on_activation: 0,
            },
            LayerConfig {
                layer: PriorityLayer::Ambient,
                allowed_mask_when_active: channels::MASK_ALL,
                lock_on_activation: false,
                lock_frames_on_activation: 0,
            },
        ];

        let arbiter = Arbiter::new(layers, 0.1);

        Self {
            registry,
            input_map,
            arbiter,
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

        self.gui.init(&window);

        let mut renderer = pollster::block_on(Renderer::new(&window));
        let mut world = World::new();

        scene::setup_default_world(&mut world);
        let host_interface = host::create_interface();

        let mut plugin_manager = PluginManager::new("target/debug/game_plugin.dll");
        plugin_manager.initial_load(&mut world, &host_interface);

        let mut active_keys: Vec<KeyCode> = Vec::new();

        event_loop
            .run(move |event, elwt| {
                elwt.set_control_flow(ControlFlow::Poll);

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
                                if let PhysicalKey::Code(KeyCode::F5) = key_event.physical_key {
                                    plugin_manager.try_hot_reload(&mut world, &host_interface);
                                }
                            }

                            if self.gui.wants_keyboard_input() {
                                return;
                            }

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
                            let mut inspector_open = self.gui.show_inspector;
                            let (primitives, textures_delta) = self.gui.draw(&window, |ctx| {
                                inspector::show(ctx, &self.arbiter, &mut inspector_open);

                                // Runtime Error Display
                                if let PluginRuntimeState::PausedError(msg) =
                                    &plugin_manager.runtime_state
                                {
                                    egui::Window::new("CRITICAL ERROR")
                                        .default_pos([400.0, 100.0])
                                        .show(ctx, |ui| {
                                            ui.colored_label(
                                                egui::Color32::RED,
                                                format!("Plugin Error: {}", msg),
                                            );
                                            ui.label("Fix source code and press F5 to reload.");
                                        });
                                }
                            });
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

                        if active_keys.contains(&KeyCode::KeyP) {
                            self.arbiter.add_movement(MovementSignal {
                                layer: PriorityLayer::Reflex,
                                vector: Vec2::ZERO,
                                weight: 1.0,
                            });
                            // Reflex also suppresses via layer config
                        }

                        let final_input_state = self.arbiter.resolve();
                        plugin_manager.update(&mut world, &final_input_state, dt);
                        window.request_redraw();
                    }
                    _ => (),
                }
            })
            .unwrap();
    }
}
