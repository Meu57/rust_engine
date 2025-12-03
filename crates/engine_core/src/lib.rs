// crates/engine_core/src/lib.rs
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use glam::Vec2;
use winit::{
    event::{Event, WindowEvent, ElementState},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
    keyboard::{KeyCode, PhysicalKey},
};

mod hot_reload;
mod renderer; 
use renderer::Renderer;

use engine_shared::{
    ActionId, ACTION_NOT_FOUND, HostInterface, InputState, MAX_AXES,
    CTransform, CSprite, CPlayer, CEnemy,
    PriorityLayer, MovementSignal, ActionSignal,
};

use engine_ecs::World; 

// --- 1. ACTION REGISTRY (Unchanged) ---
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

// --- 2. INPUT MAPPING (Unchanged) ---
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

// --- 3. THE ARBITER (With Accessors for GUI) ---
#[derive(Default)]
struct Arbiter {
    move_signals: Vec<MovementSignal>,
    action_signals: Vec<ActionSignal>,
}

impl Arbiter {
    fn clear(&mut self) {
        self.move_signals.clear();
        self.action_signals.clear();
    }

    fn add_movement(&mut self, signal: MovementSignal) {
        self.move_signals.push(signal);
    }

    fn add_action(&mut self, signal: ActionSignal) {
        self.action_signals.push(signal);
    }

    fn resolve(&self) -> InputState {
        let mut state = InputState::default();

        // A. Resolve Movement
        let mut winning_move_layer = PriorityLayer::Ambient;
        for &layer in &[PriorityLayer::Reflex, PriorityLayer::Cutscene, PriorityLayer::Control] {
            let has_signal = self.move_signals.iter().any(|s| s.layer == layer);
            if has_signal {
                winning_move_layer = layer;
                break;
            }
        }

        let mut final_vector = Vec2::ZERO;
        for s in &self.move_signals {
            if s.layer == winning_move_layer {
                final_vector += s.vector * s.weight;
            }
        }

        if final_vector.length_squared() > 1.0 {
            final_vector = final_vector.normalize();
        }
        
        state.analog_axes[0] = final_vector.x;
        state.analog_axes[1] = final_vector.y;

        // B. Resolve Actions (Digital)
        let mut winning_action_layer = PriorityLayer::Ambient;
         for &layer in &[PriorityLayer::Reflex, PriorityLayer::Cutscene, PriorityLayer::Control] {
            let has_signal = self.action_signals.iter().any(|s| s.layer == layer);
            if has_signal {
                winning_action_layer = layer;
                break; 
            }
        }

        for s in &self.action_signals {
            if s.layer == winning_action_layer && s.active {
                if (s.action_id as usize) < 64 {
                     state.digital_mask |= 1u64 << s.action_id;
                }
            }
        }

        state
    }
}

// --- 4. GLOBAL ACCESS (FFI) ---
static GLOBAL_REGISTRY: OnceLock<Mutex<ActionRegistry>> = OnceLock::new();

extern "C" fn host_get_action_id(name_ptr: *const u8, name_len: usize) -> ActionId {
    unsafe {
        if name_ptr.is_null() || name_len == 0 { return ACTION_NOT_FOUND; }
        let slice = std::slice::from_raw_parts(name_ptr, name_len);
        if let Ok(name) = std::str::from_utf8(slice) {
            if let Some(mutex) = GLOBAL_REGISTRY.get() {
                if let Ok(reg) = mutex.lock() {
                    return reg.get_id(name).unwrap_or(ACTION_NOT_FOUND);
                }
            }
        }
    }
    ACTION_NOT_FOUND
}

// --- 5. MAIN APP ---
pub struct App {
    registry: ActionRegistry,
    input_map: InputMap,
    arbiter: Arbiter,
    window_title: String,
    // GUI State
    egui_ctx: egui::Context,
    egui_winit: Option<egui_winit::State>,
}

impl App {
    pub fn new() -> Self {
        let mut registry = ActionRegistry::default();
        let mut input_map = InputMap::default();

        let move_up = registry.register("MoveUp");
        let move_down = registry.register("MoveDown");
        let move_left = registry.register("MoveLeft");
        let move_right = registry.register("MoveRight");

        input_map.bind(KeyCode::KeyW, move_up);
        input_map.bind(KeyCode::KeyS, move_down);
        input_map.bind(KeyCode::KeyA, move_left);
        input_map.bind(KeyCode::KeyD, move_right);

        let _ = GLOBAL_REGISTRY.set(Mutex::new(registry.clone()));

        Self { 
            registry, 
            input_map, 
            arbiter: Arbiter::default(),
            window_title: "Rust Engine: Input Inspector".to_string(),
            egui_ctx: egui::Context::default(),
            egui_winit: None,
        }
    }

    pub fn run(mut self) {
        let event_loop = EventLoop::new().unwrap(); 
        let window = WindowBuilder::new()
            .with_title(&self.window_title)
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0))
            .build(&event_loop)
            .unwrap();

        // Initialize EGUI winit state
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
        world.add_component(player, CTransform { pos: Vec2::new(100.0,100.0), ..Default::default() });
        world.add_component(player, CPlayer);
        world.add_component(player, CSprite::default());

        let plugin_path = "target/debug/game_plugin.dll";
        let mut game_plugin = unsafe {
            hot_reload::GamePlugin::load(plugin_path).expect("Failed to load plugin")
        };

        let host_interface = HostInterface {
            get_action_id: host_get_action_id,
            log: None,
        };

        game_plugin.api.on_load(&mut world, &host_interface);

        let mut active_keys: Vec<KeyCode> = Vec::new();

        event_loop.run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Poll); 

            // Pass event to GUI first
            if let Some(gui) = &mut self.egui_winit {
                if let Event::WindowEvent { event: ref w_event, .. } = event {
                    let _ = gui.on_window_event(&window, w_event);
                }
            }

            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => elwt.exit(),
                    
                    WindowEvent::KeyboardInput { event: key_event, .. } => {
                        // Prevent game from reading input if GUI wants it (simple check)
                        if self.egui_ctx.wants_keyboard_input() { return; }

                        if let PhysicalKey::Code(keycode) = key_event.physical_key {
                            if key_event.state == ElementState::Pressed {
                                if !active_keys.contains(&keycode) { active_keys.push(keycode); }
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

                        // --- DRAW INSPECTOR ---
                        egui::Window::new("Input Inspector")
                            .default_pos([10.0, 10.0])
                            .show(&self.egui_ctx, |ui| {
                                ui.heading("Arbitration Stack");
                                ui.separator();

                                // Visualize Layers
                                let layers = [
                                    (PriorityLayer::Reflex, "Layer 0: Reflex (Physics)", egui::Color32::RED),
                                    (PriorityLayer::Cutscene, "Layer 1: Cutscene", egui::Color32::YELLOW),
                                    (PriorityLayer::Control, "Layer 2: Player Control", egui::Color32::GREEN),
                                    (PriorityLayer::Ambient, "Layer 3: Ambient", egui::Color32::GRAY),
                                ];

                                // Find winners
                                let mut winning_move = PriorityLayer::Ambient;
                                for &(layer, _, _) in &layers {
                                    if self.arbiter.move_signals.iter().any(|s| s.layer == layer) {
                                        winning_move = layer;
                                        break; 
                                    }
                                }

                                for (layer, label, color) in layers {
                                    let is_active = self.arbiter.move_signals.iter().any(|s| s.layer == layer);
                                    let is_winner = layer == winning_move && is_active;

                                    if is_winner {
                                        ui.colored_label(color, format!("▶ {} [WINNER]", label));
                                        // Show signals
                                        for s in self.arbiter.move_signals.iter().filter(|s| s.layer == layer) {
                                            ui.label(format!("   Vector: {:.2}, Weight: {:.2}", s.vector, s.weight));
                                        }
                                    } else if is_active {
                                        ui.colored_label(color.linear_multiply(0.5), format!("▷ {} [SUPPRESSED]", label));
                                    } else {
                                        ui.label(format!("  {}", label));
                                    }
                                }
                                
                                ui.separator();
                                ui.label("Press 'P' to simulate Reflex Layer override.");
                            });

                        // --- GUI FRAME END ---
                        let gui_output = self.egui_ctx.end_frame();
                        let primitives = self.egui_ctx.tessellate(gui_output.shapes, gui_output.pixels_per_point);
                        let textures_delta = gui_output.textures_delta;

                        self.egui_winit.as_mut().unwrap().handle_platform_output(&window, gui_output.platform_output);

                        let _ = renderer.render(&world, Some((&self.egui_ctx, &primitives, &textures_delta))); 
                    }
                    _ => (),
                },
                Event::AboutToWait => {
                    let dt = 1.0 / 60.0;

                    // --- ARBITRATION LOGIC (Same as Phase 2) ---
                    self.arbiter.clear();

                    // Layer 2: Player
                    let mut player_move = Vec2::ZERO;
                    let mut player_active = false;
                    for &key in &active_keys {
                        if let Some(action_id) = self.input_map.map_signal_to_intent(key) {
                            self.arbiter.add_action(ActionSignal {
                                layer: PriorityLayer::Control,
                                action_id,
                                active: true,
                            });
                            // Basic mapping for demo
                            let id_up = self.registry.get_id("MoveUp").unwrap_or(u32::MAX);
                            let id_down = self.registry.get_id("MoveDown").unwrap_or(u32::MAX);
                            let id_left = self.registry.get_id("MoveLeft").unwrap_or(u32::MAX);
                            let id_right = self.registry.get_id("MoveRight").unwrap_or(u32::MAX);

                            if action_id == id_up { player_move.y += 1.0; player_active = true; }
                            if action_id == id_down { player_move.y -= 1.0; player_active = true; }
                            if action_id == id_left { player_move.x -= 1.0; player_active = true; }
                            if action_id == id_right { player_move.x += 1.0; player_active = true; }
                        }
                    }
                    if player_active {
                        self.arbiter.add_movement(MovementSignal {
                            layer: PriorityLayer::Control,
                            vector: player_move,
                            weight: 1.0,
                        });
                    }

                    // Layer 0: Reflex (Stun)
                    if active_keys.contains(&KeyCode::KeyP) {
                        self.arbiter.add_movement(MovementSignal {
                            layer: PriorityLayer::Reflex,
                            vector: Vec2::ZERO, 
                            weight: 1.0,
                        });
                        // Also add an action blocker
                        self.arbiter.add_action(ActionSignal {
                            layer: PriorityLayer::Reflex,
                            action_id: 0,
                            active: false,
                        });
                    }

                    let final_input_state = self.arbiter.resolve();

                    // Compatibility Map (Vector -> Bits)
                    let mut compat_state = final_input_state;
                    let vx = compat_state.analog_axes[0];
                    let vy = compat_state.analog_axes[1];
                    let id_up = self.registry.get_id("MoveUp").unwrap_or(u32::MAX);
                    let id_down = self.registry.get_id("MoveDown").unwrap_or(u32::MAX);
                    let id_left = self.registry.get_id("MoveLeft").unwrap_or(u32::MAX);
                    let id_right = self.registry.get_id("MoveRight").unwrap_or(u32::MAX);

                    if vy > 0.1 { compat_state.digital_mask |= 1 << id_up; }
                    if vy < -0.1 { compat_state.digital_mask |= 1 << id_down; }
                    if vx < -0.1 { compat_state.digital_mask |= 1 << id_left; }
                    if vx > 0.1 { compat_state.digital_mask |= 1 << id_right; }

                    game_plugin.api.update(&mut world, &compat_state, dt);
                    window.request_redraw();
                }
                _ => (),
            }
        }).unwrap();
    }
}