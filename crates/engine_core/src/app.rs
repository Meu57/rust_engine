// crates/engine_core/src/app.rs
#![allow(dead_code)]

use std::sync::Mutex;

use glam::Vec2;
use winit::{
    event::{ElementState, Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};

use crate::hot_reload;
use crate::renderer::Renderer;
use crate::input::{self, ActionRegistry, Arbiter, InputMap};
use crate::inspector; // inspector UI module

use engine_shared::{
    CEnemy, CPlayer, CSprite, CTransform, HostInterface, PriorityLayer,
    ActionSignal, MovementSignal,
};
use engine_ecs::World;

pub struct App {
    registry: ActionRegistry,
    input_map: InputMap,
    arbiter: Arbiter,
    window_title: String,
    // GUI State
    egui_ctx: egui::Context,
    egui_winit: Option<egui_winit::State>,
    show_inspector: bool,
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

        // Bind using logical keys for now (exposed API: bind_logical)
        // You can add physical bindings separately via input_map.bind_physical(...)
        input_map.bind_logical(KeyCode::KeyW, move_up);
        input_map.bind_logical(KeyCode::KeyS, move_down);
        input_map.bind_logical(KeyCode::KeyA, move_left);
        input_map.bind_logical(KeyCode::KeyD, move_right);

        // Initialize the global registry for FFI
        let _ = input::GLOBAL_REGISTRY.set(Mutex::new(registry.clone()));

        Self {
            registry,
            input_map,
            arbiter: Arbiter::default(),
            window_title: "Rust Engine: Input Inspector".to_string(),
            egui_ctx: egui::Context::default(),
            egui_winit: None,
            show_inspector: true,
        }
    }

    pub fn run(mut self) {
        let event_loop = EventLoop::new().unwrap();
        let window = WindowBuilder::new()
            .with_title(&self.window_title)
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0))
            .build(&event_loop)
            .unwrap();

        // Initialize egui_winit state (use current scale factor)
        self.egui_winit = Some(egui_winit::State::new(
            self.egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
        ));

        let mut renderer = pollster::block_on(Renderer::new(&window));
        let mut world = World::new();

        world.register_component::<CTransform>();
        world.register_component::<CPlayer>();
        world.register_component::<CEnemy>();
        world.register_component::<CSprite>();

        let player = world.spawn();
        world.add_component(
            player,
            CTransform {
                pos: Vec2::new(100.0, 100.0),
                ..Default::default()
            },
        );
        world.add_component(player, CPlayer);
        world.add_component(player, CSprite::default());

        let plugin_path = "target/debug/game_plugin.dll";
        let mut game_plugin =
            unsafe { hot_reload::GamePlugin::load(plugin_path).expect("Failed to load plugin") };

        let host_interface = HostInterface {
            get_action_id: input::host_get_action_id,
            log: None,
        };

        game_plugin.api.on_load(&mut world, &host_interface);

        // Track held logical keys (KeyCode). If you want to track physical keys separately,
        // you can push PhysicalKey values into a second list.
        let mut active_keys: Vec<KeyCode> = Vec::new();

        event_loop.run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Poll);

            // Pass window events to egui first
            if let Some(gui_state) = &mut self.egui_winit {
                if let Event::WindowEvent { event: ref w_event, .. } = event {
                    let _ = gui_state.on_window_event(&window, w_event);
                }
            }

            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => elwt.exit(),

                    WindowEvent::KeyboardInput { event: key_event, .. } => {
                        // Toggle inspector on F1 (using physical key)
                        if key_event.state == ElementState::Pressed {
                            if let PhysicalKey::Code(KeyCode::F1) = key_event.physical_key {
                                self.show_inspector = !self.show_inspector;
                            }
                        }

                        // Let egui capture keyboard first if it wants it
                        if self.egui_ctx.wants_keyboard_input() {
                            return;
                        }

                        // Track held keys (logical KeyCode)
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
                        // --- GUI FRAME START ---
                        let raw_input = self.egui_winit.as_mut().unwrap().take_egui_input(&window);
                        self.egui_ctx.begin_frame(raw_input);

                        // Inspector
                        inspector::show(&self.egui_ctx, &self.arbiter, &mut self.show_inspector);

                        // --- GUI FRAME END ---
                        let gui_output = self.egui_ctx.end_frame();

                        // Tessellate with the exact pixels_per_point that egui used.
                        let primitives = self.egui_ctx.tessellate(
                            gui_output.shapes,
                            gui_output.pixels_per_point,
                        );
                        let textures_delta = gui_output.textures_delta;

                        self.egui_winit
                            .as_mut()
                            .unwrap()
                            .handle_platform_output(&window, gui_output.platform_output);

                        // Note: renderer.render signature expects screen/egui data.
                        let _ = renderer.render(&world, Some((&self.egui_ctx, &primitives, &textures_delta)));
                    }

                    _ => (),
                },

                Event::AboutToWait => {
                    let dt = 1.0 / 60.0;

                    // Clear arbiter buffers each frame
                    self.arbiter.clear();

                    // --- Layer 2: Player Input Logic (Control) ---
                    let mut player_move = Vec2::ZERO;
                    let mut player_active = false;

                    for &key in &active_keys {
                        // Create a PhysicalKey for intent resolution.
                        // NOTE: this uses a PhysicalKey::Code fallback; ideally you'd maintain physical keys separately.
                        let physical = PhysicalKey::Code(key);

                        // Resolve intent using either logical or physical mapping.
                        // The InputMap API is expected to handle Option<KeyCode> + PhysicalKey.
                        if let Some(action_id) = self.input_map.map_signal_to_intent(Some(key), physical) {
                            self.arbiter.add_action(ActionSignal {
                                layer: PriorityLayer::Control,
                                action_id,
                                active: true,
                            });

                            let id_up = self.registry.get_id("MoveUp").unwrap_or(u32::MAX);
                            let id_down = self.registry.get_id("MoveDown").unwrap_or(u32::MAX);
                            let id_left = self.registry.get_id("MoveLeft").unwrap_or(u32::MAX);
                            let id_right = self.registry.get_id("MoveRight").unwrap_or(u32::MAX);

                            if action_id == id_up {
                                player_move.y += 1.0;
                                player_active = true;
                            }
                            if action_id == id_down {
                                player_move.y -= 1.0;
                                player_active = true;
                            }
                            if action_id == id_left {
                                player_move.x -= 1.0;
                                player_active = true;
                            }
                            if action_id == id_right {
                                player_move.x += 1.0;
                                player_active = true;
                            }
                        }
                    }

                    if player_active {
                        self.arbiter.add_movement(MovementSignal {
                            layer: PriorityLayer::Control,
                            vector: player_move,
                            weight: 1.0,
                        });
                    }

                    // --- Layer 0: Reflex (Stun) demo ---
                    if active_keys.contains(&KeyCode::KeyP) {
                        self.arbiter.add_movement(MovementSignal {
                            layer: PriorityLayer::Reflex,
                            vector: Vec2::ZERO,
                            weight: 1.0,
                        });

                        // Add an empty reflex action to force layer dominance for actions
                        self.arbiter.add_action(ActionSignal {
                            layer: PriorityLayer::Reflex,
                            action_id: 0,
                            active: false,
                        });
                    }

                    // Resolve arbiter into final InputState
                    let final_input_state = self.arbiter.resolve();

                    // Backwards compatibility mapping: analog -> digital bits
                    let mut compat_state = final_input_state;
                    let vy = compat_state.analog_axes[1];
                    let vx = compat_state.analog_axes[0];
                    let id_up = self.registry.get_id("MoveUp").unwrap_or(u32::MAX);
                    let id_down = self.registry.get_id("MoveDown").unwrap_or(u32::MAX);
                    let id_left = self.registry.get_id("MoveLeft").unwrap_or(u32::MAX);
                    let id_right = self.registry.get_id("MoveRight").unwrap_or(u32::MAX);

                    if vy > 0.1 {
                        compat_state.digital_mask |= 1 << id_up;
                    }
                    if vy < -0.1 {
                        compat_state.digital_mask |= 1 << id_down;
                    }
                    if vx < -0.1 {
                        compat_state.digital_mask |= 1 << id_left;
                    }
                    if vx > 0.1 {
                        compat_state.digital_mask |= 1 << id_right;
                    }

                    // Send resolved state to plugin
                    game_plugin.api.update(&mut world, &compat_state, dt);

                    // Request redraw after update
                    window.request_redraw();
                }

                _ => (),
            }
        })
        .unwrap();
    }
}
