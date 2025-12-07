// crates/engine_core/src/renderer/mod.rs
pub mod context;
pub mod types;
pub mod sprite_pass;

use winit::window::Window;
use engine_ecs::World;
use self::context::GraphicsContext;
use self::sprite_pass::SpritePass;

pub struct Renderer {
    ctx: GraphicsContext,
    sprite_pass: SpritePass,
    pub gui_renderer: egui_wgpu::Renderer,
}

impl Renderer {
    pub async fn new(window: &Window) -> Self {
        let ctx = GraphicsContext::new(window).await;
        let sprite_pass = SpritePass::new(&ctx);
        let gui_renderer = egui_wgpu::Renderer::new(&ctx.device, ctx.config.format, None, 1);

        Self {
            ctx,
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
        let output = self.ctx.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        // 1. Draw Game
        self.sprite_pass.draw(&self.ctx, &mut encoder, &view, world);

        // 2. Draw GUI
        if let Some((ctx, primitives, delta)) = gui_ctx {
            for (id, image_delta) in &delta.set {
                self.gui_renderer
                    .update_texture(&self.ctx.device, &self.ctx.queue, *id, image_delta);
            }

            let screen_descriptor = egui_wgpu::ScreenDescriptor {
                size_in_pixels: [self.ctx.config.width, self.ctx.config.height],
                pixels_per_point: ctx.pixels_per_point(),
            };

            self.gui_renderer.update_buffers(
                &self.ctx.device,
                &self.ctx.queue,
                &mut encoder,
                primitives,
                &screen_descriptor,
            );

            {
                let mut gui_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Gui Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load, // Load the game pass result
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                self.gui_renderer
                    .render(&mut gui_pass, primitives, &screen_descriptor);
            }

            for id in &delta.free {
                self.gui_renderer.free_texture(id);
            }
        }

        // Submit and Cleanup
        self.ctx.queue.submit(std::iter::once(encoder.finish()));
        
        // Now it's safe to recall the staging belt memory for next frame
        self.sprite_pass.cleanup();
        
        output.present();
        Ok(())
    }
}