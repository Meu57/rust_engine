// crates/engine_core/src/renderer/mod.rs

pub mod context;
pub mod types;
pub mod sprite_pass;
mod resources;
mod frame_graph;

pub use resources::RenderResources;

use winit::window::Window;
use engine_ecs::World;

use self::context::GraphicsContext;
use self::sprite_pass::SpritePass;
use self::frame_graph::{
    FrameGraph, FrameInputs, RenderPassNode, SceneToBackbufferPass, PassKind,
    PhysicalResources, PassDesc,
};

// Small adapter to treat egui_wgpu::Renderer as a RenderPassNode for the GUI pass.
struct GuiPass<'a> {
    renderer: &'a mut egui_wgpu::Renderer,
}

impl<'a> RenderPassNode for GuiPass<'a> {
    fn kind(&self) -> PassKind {
        PassKind::Gui
    }

    fn execute<'b>(
        &mut self,
        ctx: &'b GraphicsContext,
        encoder: &mut wgpu::CommandEncoder,
        resources: &PhysicalResources<'b>,
        inputs: &FrameInputs<'b>,
        pass_desc: &PassDesc,
        _pass_index: usize,
    ) {
        let Some((egui_ctx, primitives, delta)) = inputs.gui else {
            return; // GUI disabled this frame
        };

        encoder.push_debug_group(pass_desc.name);

        // Upload textures created this frame
        for (id, image_delta) in &delta.set {
            self.renderer
                .update_texture(&ctx.device, &ctx.queue, *id, image_delta);
        }

        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [ctx.config.width, ctx.config.height],
            pixels_per_point: egui_ctx.pixels_per_point(),
        };

        self.renderer.update_buffers(
            &ctx.device,
            &ctx.queue,
            encoder,
            primitives,
            &screen_descriptor,
        );

        {
            let mut gui_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Gui Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: resources.backbuffer_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            self.renderer
                .render(&mut gui_pass, primitives, &screen_descriptor);
        }

        for id in &delta.free {
            self.renderer.free_texture(id);
        }

        encoder.pop_debug_group();
    }
}

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

        // Shared GPU layouts created once
        let resources = RenderResources::new(&ctx.device);

        // Pass shared layouts into the pass
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
        let graph = FrameGraph { ctx: &self.ctx };
        let inputs = FrameInputs { world, gui: gui_ctx };

        // ----- PASSES -----
        // SpritePass lives in Self, so borrow directly.
        // Blit pass is stateless; create temporarily.
        // GUI pass borrows gui_renderer mutably.

        let mut blit_pass = SceneToBackbufferPass;
        let mut gui_pass = GuiPass {
            renderer: &mut self.gui_renderer,
        };

        // The compiler AUTOMATICALLY coerces:
        // &mut SpritePass → &mut dyn RenderPassNode
        // &mut SceneToBackbufferPass → &mut dyn RenderPassNode
        // &mut GuiPass → &mut dyn RenderPassNode

        let mut nodes: [&mut dyn RenderPassNode; 3] = [
            &mut self.sprite_pass,
            &mut blit_pass,
            &mut gui_pass,
        ];

        // Execute passes through the FrameGraph
        graph.run(&mut nodes[..], inputs)?;

        // After submission → recall StagingBelt
        self.sprite_pass.cleanup();

        Ok(())
    }
}
