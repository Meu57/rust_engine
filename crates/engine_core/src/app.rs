// crates/engine_core/src/app.rs

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;
use std::time::Instant;

use glam::Vec2;
use winit::{
    event::{ElementState, Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};

use crate::gui::GuiSystem;
use crate::host;
use crate::input::{self, ActionRegistry, Arbiter, InputMap};
use crate::input::arbiter::{channels, ActionSignal, LayerConfig, MovementSignal};
use crate::inspector;
use crate::plugin_manager::{PluginManager, PluginRuntimeState};
use crate::renderer::Renderer;
use crate::scene;

use engine_ecs::World;
use engine_shared::input_types::{canonical_actions, ActionId, InputState, PriorityLayer};

pub struct App {
    registry: ActionRegistry,
    input_map: InputMap,
    arbiter: Arbiter,
    window_title: String,
    gui: GuiSystem,

    // Engine actions routed through the Arbiter
    engine_toggle_inspector: ActionId,
    engine_request_hot_reload: ActionId,

    // Store previous frame's input for edge detection
    last_input_state: InputState,

    // Configurable plugin path (not hardcoded)
    plugin_path: String,
}

// Simple, best-effort file logger for fatal errors.
fn log_fatal_error_to_file(message: &str) {
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("engine_fatal.log")
    {
        let _ = writeln!(file, "{}", message);
    }
}

impl App {
    /// Create a new App with a configurable plugin path.
    pub fn new(plugin_path: &str) -> Self {
        let mut registry = ActionRegistry::default();
        let mut input_map = InputMap::default();

        // 1. Register canonical movement actions FIRST to guarantee IDs 0..3
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

        // 2. Register Engine actions as first-class actions
        let engine_toggle_inspector = registry.register("Engine.ToggleInspector");
        let engine_request_hot_reload = registry.register("Engine.RequestHotReload");

        // Bind F1/F5 to engine actions instead of hardcoding them
        input_map.bind_logical(KeyCode::F1, engine_toggle_inspector);
        input_map.bind_logical(KeyCode::F5, engine_request_hot_reload);

        // Publish registry globally for plugins / tools
        let _ = input::GLOBAL_REGISTRY.set(Mutex::new(registry.clone()));

        // 3. Configure Arbiter layers
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

            engine_toggle_inspector,
            engine_request_hot_reload,

            // start with no actions pressed
            last_input_state: InputState::default(),

            plugin_path: plugin_path.to_string(),
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

        let mut plugin_manager = PluginManager::new(&self.plugin_path);
        plugin_manager.initial_load(&mut world, &host_interface);

        let mut active_keys: Vec<KeyCode> = Vec::new();

        // Fixed-timestep simulation bookkeeping
        let mut last_frame_time = Instant::now();
        let mut sim_accumulator: f32 = 0.0;
        const SIM_DT: f32 = 1.0 / 60.0;
        const MAX_STEPS_PER_FRAME: u32 = 5;

        event_loop
            .run(move |event, elwt| {
                elwt.set_control_flow(ControlFlow::Poll);

                if let Event::WindowEvent {
                    event: ref w_event,
                    ..
                } = event
                {
                    self.gui.handle_event(&window, w_event);
                }

                match event {
                    Event::WindowEvent { event, .. } => match event {
                        WindowEvent::CloseRequested => elwt.exit(),

                        WindowEvent::KeyboardInput { event: key_event, .. } => {
                            // NOTE: F1/F5 now go through input_map/arbiter only.
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

                            // Robust SurfaceError handling
                            match renderer.render(
                                &world,
                                Some((&self.gui.ctx, &primitives, &textures_delta)),
                            ) {
                                Ok(()) => {
                                    // all good
                                }
                                Err(wgpu::SurfaceError::Lost)
                                | Err(wgpu::SurfaceError::Outdated) => {
                                    // Surface was lost or outdated (e.g., window resize, alt-tab).
                                    // Reconfigure swapchain and continue.
                                    eprintln!(
                                        "[Renderer] Surface lost/outdated. Reconfiguring swapchain."
                                    );
                                    renderer.resize(window.inner_size());
                                }
                                Err(wgpu::SurfaceError::OutOfMemory) => {
                                    // This is a *fatal* error. Continuing would be undefined behavior.
                                    let msg = "[Renderer] FATAL: Out of GPU memory. Exiting.";
                                    eprintln!("{msg}");
                                    log_fatal_error_to_file(msg);
                                    elwt.exit();
                                }
                                Err(wgpu::SurfaceError::Timeout) => {
                                    // Non-fatal; just log and keep going.
                                    eprintln!("[Renderer] Surface timeout. Skipping this frame.");
                                }
                            }
                        }
                        _ => (),
                    },

                    Event::AboutToWait => {
                        // --- Fixed-timestep simulation loop ---

                        // 1) Measure frame time and update accumulator
                        let now = Instant::now();
                        let frame_dt =
                            now.duration_since(last_frame_time).as_secs_f32();
                        last_frame_time = now;

                        // Clamp to avoid huge spikes (e.g. window dragged, breakpoint)
                        let frame_dt = frame_dt.min(0.25);
                        sim_accumulator += frame_dt;

                        // 2) Clear arbiter and feed active keys
                        self.arbiter.clear();

                        // 1. Convert active_keys → logical actions via InputMap
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

                        // Optional: Reflex test
                        if active_keys.contains(&KeyCode::KeyP) {
                            self.arbiter.add_movement(MovementSignal {
                                layer: PriorityLayer::Reflex,
                                vector: Vec2::ZERO,
                                weight: 1.0,
                            });
                            // Reflex suppression is handled by layer config.
                        }

                        // 3) Let arbiter resolve final InputState
                        let final_input_state = self.arbiter.resolve();

                        // 4) ENGINE ACTIONS — edge-triggered
                        let toggle_now =
                            final_input_state
                                .is_active(self.engine_toggle_inspector)
                                && !self
                                    .last_input_state
                                    .is_active(self.engine_toggle_inspector);

                        let reload_now =
                            final_input_state
                                .is_active(self.engine_request_hot_reload)
                                && !self
                                    .last_input_state
                                    .is_active(self.engine_request_hot_reload);

                        if toggle_now {
                            self.gui.toggle_inspector();
                        }

                        if reload_now {
                            plugin_manager.try_hot_reload(&mut world, &host_interface);
                        }

                        // 5) Run fixed-step simulation updates
                        let mut steps = 0;
                        while sim_accumulator >= SIM_DT && steps < MAX_STEPS_PER_FRAME {
                            plugin_manager.update(
                                &mut world,
                                &final_input_state,
                                SIM_DT,
                            );
                            sim_accumulator -= SIM_DT;
                            steps += 1;
                        }

                        // Optional but recommended: prevent unbounded backlog under heavy load.
                        if steps == MAX_STEPS_PER_FRAME && sim_accumulator >= SIM_DT {
                            // We are not keeping up; drop extra accumulated time
                            // so the simulation can recover instead of trying to
                            // catch up forever.
                            sim_accumulator = 0.0;
                        }

                        // 6) Update last_input_state for next frame
                        self.last_input_state = final_input_state;

                        // 7) Request a redraw
                        window.request_redraw();
                    }
                    _ => (),
                }
            })
            .unwrap();
    }
}
