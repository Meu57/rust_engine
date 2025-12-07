// crates/engine_core/src/gui.rs
use egui::Context;
use winit::{event::WindowEvent, window::Window};

pub struct GuiSystem {
    pub ctx: Context,
    // State is an Option because it requires the Window to be created first
    state: Option<egui_winit::State>,
    pub show_inspector: bool,
}

impl GuiSystem {
    pub fn new() -> Self {
        Self {
            ctx: Context::default(),
            state: None,
            show_inspector: true,
        }
    }

    /// Initialize the integration once the window exists
    pub fn init(&mut self, window: &Window) {
        self.state = Some(egui_winit::State::new(
            self.ctx.clone(),
            egui::ViewportId::ROOT,
            window,
            Some(window.scale_factor() as f32),
            None,
        ));
    }

    /// Forward window events to egui
    pub fn handle_event(&mut self, window: &Window, event: &WindowEvent) {
        if let Some(state) = &mut self.state {
            let _ = state.on_window_event(window, event);
        }
    }

    pub fn wants_keyboard_input(&self) -> bool {
        self.ctx.wants_keyboard_input()
    }

    pub fn toggle_inspector(&mut self) {
        self.show_inspector = !self.show_inspector;
    }

    /// Prepare the frame, run the UI closure, and output draw data
    pub fn draw(
        &mut self,
        window: &Window,
        run_ui: impl FnOnce(&Context),
    ) -> (Vec<egui::ClippedPrimitive>, egui::TexturesDelta) {
        let state = self.state.as_mut().expect("GuiSystem not initialized!");
        
        let raw_input = state.take_egui_input(window);
        self.ctx.begin_frame(raw_input);

        // Run the actual UI logic passed by the caller
        run_ui(&self.ctx);

        let output = self.ctx.end_frame();
        
        state.handle_platform_output(window, output.platform_output);
        
        let primitives = self.ctx.tessellate(output.shapes, output.pixels_per_point);
        (primitives, output.textures_delta)
    }
}