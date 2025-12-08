// crates/engine_core/src/renderer/frame_graph.rs

use engine_ecs::World;

use crate::renderer::context::GraphicsContext;
use crate::renderer::sprite_pass::SpritePass;

/// Inputs that the frame graph needs for one frame.
pub struct FrameInputs<'a> {
    pub world: &'a World,
    pub gui: Option<(
        &'a egui::Context,
        &'a Vec<egui::ClippedPrimitive>,
        &'a egui::TexturesDelta,
    )>,
}

/// Outputs for one frame. Empty for now, but we keep this
/// struct so we can add timing/profiling/attachments later.
pub struct FrameOutputs;

/// Minimal “frame graph” wrapper for your current passes.
/// Right now it just runs:
///   1. Sprite (game) pass
///   2. GUI pass
pub struct FrameGraph<'a> {
    pub ctx: &'a GraphicsContext,
}

impl<'a> FrameGraph<'a> {
    pub fn run(
        &self,
        sprite_pass: &mut SpritePass,
        gui_renderer: &mut egui_wgpu::Renderer,
        inputs: FrameInputs<'a>,
    ) -> Result<FrameOutputs, wgpu::SurfaceError> {
        // Acquire the backbuffer. Any surface loss / timeout / etc.
        // is reported via the SurfaceError and handled at the App level.
        let output = self.ctx.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Single command encoder for the frame
        let mut encoder =
            self.ctx
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("FrameGraph Encoder"),
                });

        // ---------------------------------------------------------------------
        // 1. Game / Sprite pass
        // ---------------------------------------------------------------------
        encoder.push_debug_group("SpritePass");
        sprite_pass.draw(self.ctx, &mut encoder, &view, inputs.world);
        encoder.pop_debug_group();

        // ---------------------------------------------------------------------
        // 2. GUI pass (reusing your existing logic)
        // ---------------------------------------------------------------------
        if let Some((ctx, primitives, delta)) = inputs.gui {
            encoder.push_debug_group("GuiPass");

            // Upload textures set this frame
            for (id, image_delta) in &delta.set {
                gui_renderer.update_texture(
                    &self.ctx.device,
                    &self.ctx.queue,
                    *id,
                    image_delta,
                );
            }

            let screen_descriptor = egui_wgpu::ScreenDescriptor {
                size_in_pixels: [self.ctx.config.width, self.ctx.config.height],
                pixels_per_point: ctx.pixels_per_point(),
            };

            gui_renderer.update_buffers(
                &self.ctx.device,
                &self.ctx.queue,
                &mut encoder,
                primitives,
                &screen_descriptor,
            );

            {
                let mut gui_pass =
                    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Gui Render Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                // Load the result of the sprite pass
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });

                gui_renderer.render(&mut gui_pass, primitives, &screen_descriptor);
            }

            // Free any textures that egui asked us to drop
            for id in &delta.free {
                gui_renderer.free_texture(id);
            }

            encoder.pop_debug_group();
        }

        // Submit work and present
        self.ctx.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(FrameOutputs)
    }
}
