// crates/engine_core/src/lib.rs

use winit::{
    event::{Event, WindowEvent, ElementState},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
    keyboard::PhysicalKey,
};

mod renderer;
mod hot_reload;

use renderer::Renderer;
use engine_ecs::World;
use engine_shared::{CTransform, CSprite, CPlayer, CEnemy, Input};
use glam::Vec2;
use tracing_subscriber;

pub struct App {
    window_title: String,
}

impl App {
    pub fn new() -> Self {
        let _ = tracing_subscriber::fmt::try_init();
        Self {
            window_title: "Rust Engine".to_string(),
        }
    }

    pub fn run(self) {
        tracing::info!("Starting Engine...");
        let event_loop = EventLoop::new().unwrap();

        let window = WindowBuilder::new()
            .with_title(&self.window_title)
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0))
            .build(&event_loop)
            .unwrap();

        let mut renderer = pollster::block_on(Renderer::new(&window));

        // ECS setup
        let mut world = World::new();

        // Register components (CRITICAL: include CEnemy)
        world.register_component::<CTransform>();
        world.register_component::<CPlayer>();
        world.register_component::<CEnemy>(); // <-- critical fix to prevent "Component not registered!"
        world.register_component::<CSprite>();

        // Spawn player (entity 0)
        let player = world.spawn();
        world.add_component(player, CTransform {
            pos: Vec2::new(100.0, 100.0),
            ..Default::default()
        });
        world.add_component(player, CPlayer);
        world.add_component(player, CSprite::default());

        // Hot-reload plugin
        let plugin_path = "target/debug/game_plugin.dll";
        let mut game_plugin = unsafe {
            hot_reload::GamePlugin::load(plugin_path)
                .expect("Failed to load game plugin!")
        };

        // on_load hook may use world
        game_plugin.api.on_load(&mut world);

        // Input resource
        let mut input = Input::default();

        tracing::info!("Engine Systems Online.");

        event_loop.run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Poll);

            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => elwt.exit(),
                    WindowEvent::Resized(physical_size) => renderer.resize(physical_size),

                    // Keyboard input handling (physical keys)
                    WindowEvent::KeyboardInput { event: key_event, .. } => {
                        if let PhysicalKey::Code(keycode) = key_event.physical_key {
                            let key_code_u32 = keycode as u32;
                            match key_event.state {
                                ElementState::Pressed => {
                                    if !input.keys_pressed.contains(&key_code_u32) {
                                        input.keys_just_pressed.insert(key_code_u32);
                                    }
                                    input.keys_pressed.insert(key_code_u32);
                                }
                                ElementState::Released => {
                                    input.keys_pressed.remove(&key_code_u32);
                                }
                            }
                        }
                    }

                    WindowEvent::RedrawRequested => {
                        let _ = renderer.render(&world);
                    }
                    _ => (),
                },

                Event::AboutToWait => {
                    let dt = 0.016_f32; // stable timestep for now

                    // update plugin/game logic
                    game_plugin.api.update(&mut world, &input, dt);

                    input.keys_just_pressed.clear();
                    window.request_redraw();
                }

                _ => (),
            }
        }).unwrap();
    }
}
