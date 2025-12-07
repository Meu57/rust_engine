// crates/engine_core/src/app.rs
#![allow(dead_code)]

use std::sync::Mutex;
use std::time::{Duration, Instant};

use glam::Vec2;
use winit::{
    event::{ElementState, Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};

// Use our new modules
use crate::hot_reload;
use crate::renderer::Renderer;
use crate::input::{self, ActionRegistry, Arbiter, InputMap};
use crate::inspector;
use crate::scene; // <--- NEW
use crate::host;  // <--- NEW

use engine_shared::{
    HostInterface, PriorityLayer, ActionSignal, MovementSignal, HostContext,
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

        input_map.bind_logical(KeyCode::KeyW, move_up);
        input_map.bind_logical(KeyCode::KeyS, move_down);
        input_map.bind_logical(KeyCode::KeyA, move_left);
        input_map.bind_logical(KeyCode::KeyD, move_right);

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

        self.egui_winit = Some(egui_winit::State::new(
            self.egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
        ));

        let mut renderer = pollster::block_on(Renderer::new(&window));
        let mut world = World::new();

        // --- NEW: Use the scene module to setup the world ---
        scene::setup_default_world(&mut world);

        // Plugin path
        let plugin_path: &'static str = "target/debug/game_plugin.dll";

        // Initial load
        let mut game_plugin =
            unsafe { hot_reload::GamePlugin::load(plugin_path).expect("Failed to load plugin") };

        // --- NEW: Use the host module to create the interface ---
        let host_interface = host::create_interface();

        // Initial negotiation with plugin (no snapshot yet).
        unsafe {
            let ok = (game_plugin.api.on_load)(
                game_plugin.api.state,
                &mut world as *mut _ as *mut HostContext,
                &host_interface as *const _,
                std::ptr::null(),
                0,
            );

            if !ok {
                eprintln!(
                    "Warning: initial plugin on_load() reported failure. \
                     Continuing with plugin default state."
                );
            }
        }

        let mut active_keys: Vec<KeyCode> = Vec::new();
        let mut last_reload: Option<Instant> = None;
        let reload_debounce = Duration::from_millis(500);

        event_loop
            .run(move |event, elwt| {
                elwt.set_control_flow(ControlFlow::Poll);

                if let Some(gui_state) = &mut self.egui_winit {
                    if let Event::WindowEvent { event: ref w_event, .. } = event {
                        let _ = gui_state.on_window_event(&window, w_event);
                    }
                }

                match event {
                    Event::WindowEvent { event, .. } => match event {
                        WindowEvent::CloseRequested => elwt.exit(),

                        WindowEvent::KeyboardInput { event: key_event, .. } => {
                            if key_event.state == ElementState::Pressed {
                                if let PhysicalKey::Code(KeyCode::F1) = key_event.physical_key {
                                    self.show_inspector = !self.show_inspector;
                                }
                            }

                            // HOT-RELOAD TRIGGER (F5)
                            if key_event.state == ElementState::Pressed {
                                if let PhysicalKey::Code(KeyCode::F5) = key_event.physical_key {
                                    let now = Instant::now();
                                    let allowed = last_reload
                                        .map(|t| now.duration_since(t) >= reload_debounce)
                                        .unwrap_or(true);

                                    if allowed {
                                        last_reload = Some(now);
                                        println!("🔄 Hot Reload requested (F5)...");

                                        // 1. SAVE STATE (Host Authority)
                                        let required_size =
                                            (game_plugin.api.get_state_size)(game_plugin.api.state);

                                        let mut snapshot_buffer = if required_size > 0 {
                                            vec![0u8; required_size]
                                        } else {
                                            Vec::new()
                                        };

                                        let written_len = if !snapshot_buffer.is_empty() {
                                            (game_plugin.api.save_state)(
                                                game_plugin.api.state,
                                                snapshot_buffer.as_mut_ptr(),
                                                snapshot_buffer.len(),
                                            )
                                        } else {
                                            0
                                        };

                                        if written_len == 0 && required_size > 0 {
                                            eprintln!("⚠️ Plugin failed to write snapshot.");
                                            snapshot_buffer.clear();
                                        } else if written_len > 0 && written_len < snapshot_buffer.len() {
                                            snapshot_buffer.truncate(written_len);
                                        }

                                        // 2. ATOMIC SWAP HANDSHAKE
                                        unsafe {
                                            match hot_reload::GamePlugin::load(plugin_path) {
                                                Ok(mut new_plugin) => {
                                                    println!("Verifying new plugin...");
                                                    let success = (new_plugin.api.on_load)(
                                                        new_plugin.api.state,
                                                        &mut world as *mut _ as *mut HostContext,
                                                        &host_interface as *const _,
                                                        if snapshot_buffer.is_empty() {
                                                            std::ptr::null()
                                                        } else {
                                                            snapshot_buffer.as_ptr()
                                                        },
                                                        snapshot_buffer.len(),
                                                    );

                                                    if success {
                                                        game_plugin = new_plugin;
                                                        println!("✅ Hot Reload Success! State preserved.");
                                                    } else {
                                                        eprintln!("❌ New plugin rejected the snapshot. Keeping old plugin.");
                                                    }
                                                }
                                                Err(e) => {
                                                    eprintln!("❌ Hot Reload Failed (Load Error): {e}");
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            if self.egui_ctx.wants_keyboard_input() {
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
                            let raw_input =
                                self.egui_winit.as_mut().unwrap().take_egui_input(&window);
                            self.egui_ctx.begin_frame(raw_input);

                            inspector::show(
                                &self.egui_ctx,
                                &self.arbiter,
                                &mut self.show_inspector,
                            );

                            let gui_output = self.egui_ctx.end_frame();
                            let primitives = self.egui_ctx.tessellate(
                                gui_output.shapes,
                                gui_output.pixels_per_point,
                            );
                            let textures_delta = gui_output.textures_delta;

                            self.egui_winit
                                .as_mut()
                                .unwrap()
                                .handle_platform_output(
                                    &window,
                                    gui_output.platform_output,
                                );

                            let _ = renderer.render(
                                &world,
                                Some((&self.egui_ctx, &primitives, &textures_delta)),
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

                        // Debug: Keep Reflex layer override for testing
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

                        unsafe {
                            (game_plugin.api.update)(
                                game_plugin.api.state,
                                &mut world as *mut _ as *mut HostContext,
                                &final_input_state as *const _,
                                dt,
                            );
                        }

                        window.request_redraw();
                    }

                    _ => (),
                }
            })
            .unwrap();
    }
}