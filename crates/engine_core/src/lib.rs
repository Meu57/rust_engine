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
    PriorityLayer, MovementSignal, ActionSignal, // <--- New Imports
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

// --- 3. THE ARBITER (New System) ---
/// The logic brain that filters conflicting signals.
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

    /// The Core "Subsumption" Logic
    fn resolve(&self) -> InputState {
        let mut state = InputState::default();

        // A. Resolve Movement
        // Rule: Identify the "Winning Layer" (Lowest Enum Value = Highest Priority)
        // that has ANY non-zero input.
        let mut winning_move_layer = PriorityLayer::Ambient;
        
        // Find highest priority layer present in the buffer
        // (Iterate layers 0..3)
        for &layer in &[PriorityLayer::Reflex, PriorityLayer::Cutscene, PriorityLayer::Control] {
            let has_signal = self.move_signals.iter().any(|s| s.layer == layer);
            if has_signal {
                winning_move_layer = layer;
                break; // Found the boss. Stop looking.
            }
        }

        // Sum vectors ONLY from the winning layer (Vector Blending)
        // [Cite: Input_Handeling_0.pdf, Page 109 - "Vector Blending Overlay"]
        let mut final_vector = Vec2::ZERO;
        for s in &self.move_signals {
            if s.layer == winning_move_layer {
                final_vector += s.vector * s.weight;
            }
        }

        // Clamp to unit circle (analog stick behavior)
        if final_vector.length_squared() > 1.0 {
            final_vector = final_vector.normalize();
        }
        
        // Write to axes (Arbitrary mapping: Axis 0=X, Axis 1=Y)
        state.analog_axes[0] = final_vector.x;
        state.analog_axes[1] = final_vector.y;


        // B. Resolve Actions (Digital)
        // Rule: Strict Layer Dominance. If Layer 0 is active, Layer 2 inputs are ignored.
        // First, find the highest priority layer that is emitting ANY action signal.
        let mut winning_action_layer = PriorityLayer::Ambient;
         for &layer in &[PriorityLayer::Reflex, PriorityLayer::Cutscene, PriorityLayer::Control] {
            let has_signal = self.action_signals.iter().any(|s| s.layer == layer);
            if has_signal {
                winning_action_layer = layer;
                break; 
            }
        }

        // Apply only bits from the winner
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
    arbiter: Arbiter, // <--- Added Arbiter
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

        // W/A/S/D bindings
        input_map.bind(KeyCode::KeyW, move_up);
        input_map.bind(KeyCode::KeyS, move_down);
        input_map.bind(KeyCode::KeyA, move_left);
        input_map.bind(KeyCode::KeyD, move_right);

        // Publish snapshot for FFI
        let _ = GLOBAL_REGISTRY.set(Mutex::new(registry.clone()));

        Self { 
            registry, 
            input_map, 
            arbiter: Arbiter::default(),
            window_title: "Rust Engine: Arbitration Demo".to_string() 
        }
    }

    pub fn run(mut self) {
        let event_loop = EventLoop::new().unwrap(); 
        let window = WindowBuilder::new()
            .with_title(&self.window_title)
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0))
            .build(&event_loop)
            .unwrap();

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

        // Load Plugin
        let plugin_path = "target/debug/game_plugin.dll";
        let mut game_plugin = unsafe {
            hot_reload::GamePlugin::load(plugin_path).expect("Failed to load plugin")
        };

        let host_interface = HostInterface {
            get_action_id: host_get_action_id,
            log: None,
        };

        game_plugin.api.on_load(&mut world, &host_interface);

        // Track raw key state for the demo
        let mut active_keys: Vec<KeyCode> = Vec::new();

        event_loop.run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Poll); 

            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => elwt.exit(),
                    
                    WindowEvent::KeyboardInput { event: key_event, .. } => {
                        if let PhysicalKey::Code(keycode) = key_event.physical_key {
                            // Simple tracker to keep list of held keys
                            if key_event.state == ElementState::Pressed {
                                if !active_keys.contains(&keycode) { active_keys.push(keycode); }
                            } else {
                                active_keys.retain(|&k| k != keycode);
                            }
                        }
                    }
                    
                    WindowEvent::Resized(size) => renderer.resize(size),
                    WindowEvent::RedrawRequested => { let _ = renderer.render(&world); }
                    _ => (),
                },
                Event::AboutToWait => {
                    let dt = 1.0 / 60.0;

                    // --- STEP 1: GATHER SIGNALS ---
                    self.arbiter.clear();

                    // A. Process Layer 2 (Player Control)
                    // Convert raw W/A/S/D keys into Movement Intent
                    let mut player_move = Vec2::ZERO;
                    let mut player_active = false;

                    for &key in &active_keys {
                        if let Some(action_id) = self.input_map.map_signal_to_intent(key) {
                            // Map Actions to Bits
                            self.arbiter.add_action(ActionSignal {
                                layer: PriorityLayer::Control,
                                action_id,
                                active: true,
                            });

                            // Map Keys to Vector (Hardcoded for this demo, usually data-driven)
                            // We need to know which action ID maps to which vector direction
                            // Ideally we store this in metadata, but for now:
                            // Check Action IDs by looking up string (slow but fine for demo)
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

                    // B. Process Layer 0 (Reflex/Stun) - THE DEMO
                    // If user holds 'P', we inject a Reflex signal that overrides everything.
                    if active_keys.contains(&KeyCode::KeyP) {
                        // "P" for Punish/Pause
                        // We add a signal to Layer 0.
                        // Vector is ZERO (Freeze).
                        self.arbiter.add_movement(MovementSignal {
                            layer: PriorityLayer::Reflex,
                            vector: Vec2::ZERO, 
                            weight: 1.0,
                        });
                        
                        // Also block actions? 
                        // By adding an empty/dummy action signal to Reflex, 
                        // we force the Action Arbiter to select Reflex layer (which has 0 bits set).
                        self.arbiter.add_action(ActionSignal {
                            layer: PriorityLayer::Reflex,
                            action_id: 0, 
                            active: false, // "I am active, but I am pressing nothing."
                        });
                    }

                    // --- STEP 2: RESOLVE ---
                    let final_input_state = self.arbiter.resolve();

                    // --- STEP 3: UPDATE PLUGIN ---
                    // Note: We need to make sure the Plugin looks at Axes for movement now,
                    // or we map the axes back to bits for backward compat.
                    // For now, let's map Axes -> Bits in the Arbiter output so the old Plugin logic still works!
                    // (See Arbiter resolve: it writes bits. But wait, we calculated Vector.)
                    // Fix: The plugin expects Digital Bits for movement (MoveUp/Down).
                    // We should update the Plugin to use Analog Axes OR map the resolved vector back to bits here.
                    // Let's map Vector -> Bits for compatibility.
                    
                    let mut compat_state = final_input_state;
                    let vx = compat_state.analog_axes[0];
                    let vy = compat_state.analog_axes[1];
                    
                    // Thresholds to trigger digital buttons from analog result
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