// crates/engine_core/src/renderer/mod.rs
pub mod context;
pub mod types;
pub mod sprite_pass;
mod resources;
mod frame_graph; // <-- NEW

pub use resources::RenderResources;

use winit::window::Window;
use engine_ecs::World;

use self::context::GraphicsContext;
use self::sprite_pass::SpritePass;
use self::frame_graph::{FrameGraph, FrameInputs};

pub struct Renderer {
    ctx: GraphicsContext,
    /// Central registry of shared GPU layouts/resources.
    resources: RenderResources,
    sprite_pass: SpritePass,
    pub gui_renderer: egui_wgpu::Renderer,
}

impl Renderer {
    pub async fn new(window: &Window) -> Self {
        let ctx = GraphicsContext::new(window).await;

        // Create shared GPU layouts once, here.
        let resources = RenderResources::new(&ctx.device);

        // Pass shared layouts into the pass.
        let sprite_pass = SpritePass::new(&ctx, &resources);

        let gui_renderer =
            egui_wgpu::Renderer::new(&ctx.device, ctx.config.format, None, 1);

        Self {
            ctx,
            resources,
            sprite_pass,
            gui_renderer,
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.ctx.resize(new_size);
    }

    pub fn render(
        &mut self,
        world: &World,
        gui_ctx: Option<(
            &egui::Context,
            &Vec<egui::ClippedPrimitive>,
            &egui::TexturesDelta,
        )>,
    ) -> Result<(), wgpu::SurfaceError> {
        // Build a "frame graph" for this frame
        let graph = FrameGraph { ctx: &self.ctx };
        let inputs = FrameInputs { world, gui: gui_ctx };

        // Execute passes through the mini frame graph.
        // The graph is responsible for:
        // - acquiring the surface texture
        // - building encoder and render passes
        // - running sprite + GUI passes
        // - submitting to the queue and presenting
        let _outputs = graph.run(
            &mut self.sprite_pass,
            &mut self.gui_renderer,
            inputs,
        )?;

        // Cleanup staging belt etc. after GPU work was submitted
        self.sprite_pass.cleanup();

        Ok(())
    }
}
