// crates/engine_core/src/platform_runner.rs

use std::fs::OpenOptions;
use std::io::Write;

use glam::Vec2;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::KeyCode;
use winit::window::WindowBuilder;

use crate::app::App;
use crate::engine_loop::EngineLoop;
use crate::host;
use crate::input::arbiter::MovementSignal;
use crate::input::poller::InputPoller;
use crate::inspector;
use crate::plugin_manager::{PluginManager, PluginRuntimeState};
use crate::renderer::Renderer;
use crate::scene;

use engine_ecs::World;
use engine_shared::input_types::{InputState, PriorityLayer};
use engine_shared::plugin_api::HostInterface;

/// Simple, best-effort file logger for fatal errors.
fn log_fatal_error_to_file(message: &str) {
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("engine_fatal.log")
    {
        let _ = writeln!(file, "{}", message);
    }
}

/// Owns App and runs the platform (winit) event loop.
/// This isolates OS interaction from the engine core.
pub struct PlatformRunner {
    app: App,
}

impl PlatformRunner {
    pub fn new(app: App) -> Self {
        Self { app }
    }

    pub fn start(mut self) {
        let event_loop = EventLoop::new().unwrap();
        let window = WindowBuilder::new()
            .with_title(&self.app.window_title)
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0))
            .build(&event_loop)
            .unwrap();

        // GUI + renderer initialization
        self.app.gui.init(&window);
        let mut renderer = pollster::block_on(Renderer::new(&window));

        // ECS + plugin initialization
        let mut world = World::new();
        scene::setup_default_world(&mut world);
        let host_interface: HostInterface = host::create_interface();

        let mut plugin_manager = PluginManager::new(&self.app.plugin_path);
        plugin_manager.initial_load(&mut world, &host_interface);

        // Engine loop + input poller
        const SIM_DT: f32 = 1.0 / 60.0;
        let mut engine_loop = EngineLoop::new(SIM_DT);
        let mut input_poller = InputPoller::new();

        event_loop
            .run(move |event, elwt| {
                elwt.set_control_flow(ControlFlow::Poll);

                // Give GUI first shot at all window events (for focus, etc.).
                if let Event::WindowEvent { event: ref w_event, .. } = event {
                    self.app.gui.handle_event(&window, w_event);
                }

                match event {
                    Event::WindowEvent { event: win_event, .. } => {
                        match win_event {
                            WindowEvent::CloseRequested => elwt.exit(),

                            // Low-level input: delegate to InputPoller unless GUI owns keyboard.
                            WindowEvent::KeyboardInput { .. } => {
                                if !self.app.gui.wants_keyboard_input() {
                                    input_poller.handle_event(&win_event);
                                }
                            }

                            WindowEvent::Resized(size) => renderer.resize(size),

                            WindowEvent::RedrawRequested => {
                                // --- RENDER PHASE ---

                                let mut inspector_open = self.app.gui.show_inspector;
                                let (primitives, textures_delta) =
                                    self.app.gui.draw(&window, |ctx| {
                                        // Input inspector UI.
                                        inspector::show(
                                            ctx,
                                            &self.app.arbiter,
                                            &mut inspector_open,
                                        );

                                        // Plugin runtime errors overlay.
                                        if let PluginRuntimeState::PausedError(msg) =
                                            &plugin_manager.runtime_state
                                        {
                                            egui::Window::new("CRITICAL ERROR")
                                                .default_pos([400.0, 100.0])
                                                .show(ctx, |ui| {
                                                    ui.colored_label(
                                                        egui::Color32::RED,
                                                        format!(
                                                            "Plugin Error: {}",
                                                            msg
                                                        ),
                                                    );
                                                    ui.label(
                                                        "Fix source code and press F5 to reload.",
                                                    );
                                                });
                                        }
                                    });
                                self.app.gui.show_inspector = inspector_open;

                                // Robust surface error handling (parity with original App::run).
                                match renderer.render(
                                    &world,
                                    Some((
                                        &self.app.gui.ctx,
                                        &primitives,
                                        &textures_delta,
                                    )),
                                ) {
                                    Ok(()) => {
                                        // all good
                                    }
                                    Err(wgpu::SurfaceError::Lost)
                                    | Err(wgpu::SurfaceError::Outdated) => {
                                        eprintln!(
                                            "[Renderer] Surface lost/outdated. Reconfiguring swapchain."
                                        );
                                        renderer.resize(window.inner_size());
                                    }
                                    Err(wgpu::SurfaceError::OutOfMemory) => {
                                        let msg = "[Renderer] FATAL: Out of GPU memory. Exiting.";
                                        eprintln!("{msg}");
                                        log_fatal_error_to_file(msg);
                                        elwt.exit();
                                    }
                                    Err(wgpu::SurfaceError::Timeout) => {
                                        eprintln!(
                                            "[Renderer] Surface timeout. Skipping this frame."
                                        );
                                    }
                                }
                            }

                            _ => {}
                        }
                    }

                    Event::AboutToWait => {
                        // --- UPDATE PHASE ---

                        // 1) Time step
                        let frame_dt = engine_loop.tick_timer();

                        // 2) Input resolution: raw → Arbiter → final InputState
                        input_poller.synchronize_with_arbiter(
                            &mut self.app.arbiter,
                            &self.app.input_map,
                        );

                        // Optional Reflex test: P key triggers a Reflex-layer movement override.
                        // This preserves the original behavior from the monolithic App::run.
                        if input_poller.is_key_active(KeyCode::KeyP) {
                            self.app.arbiter.add_movement(MovementSignal {
                                layer: PriorityLayer::Reflex,
                                vector: Vec2::ZERO,
                                weight: 1.0,
                            });
                        }

                        let final_input_state = self.app.arbiter.resolve();

                        // 3) Engine internal actions (Inspector / Hot reload), edge-triggered.
                        self.handle_engine_actions(
                            &final_input_state,
                            &mut plugin_manager,
                            &mut world,
                            &host_interface,
                        );

                        // 4) Fixed-step simulation.
                        engine_loop.update_simulation(
                            frame_dt,
                            &mut world,
                            &mut plugin_manager,
                            &final_input_state,
                        );

                        // 5) Store for next-frame edge detection and request redraw.
                        self.app.last_input_state = final_input_state;
                        window.request_redraw();
                    }

                    _ => {}
                }
            })
            .unwrap();
    }

    /// Edge-triggered engine actions (Inspector toggle, Hot reload),
    /// split out to keep the main loop readable.
    fn handle_engine_actions(
        &mut self,
        current_state: &InputState,
        plugin_manager: &mut PluginManager,
        world: &mut World,
        host_interface: &HostInterface,
    ) {
        let toggle_now = current_state.is_active(self.app.engine_toggle_inspector)
            && !self
                .app
                .last_input_state
                .is_active(self.app.engine_toggle_inspector);

        let reload_now = current_state.is_active(self.app.engine_request_hot_reload)
            && !self
                .app
                .last_input_state
                .is_active(self.app.engine_request_hot_reload);

        if toggle_now {
            self.app.gui.toggle_inspector();
        }

        if reload_now {
            plugin_manager.try_hot_reload(world, host_interface);
        }
    }
}
