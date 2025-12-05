// crates/engine_core/src/renderer/gui.rs
use egui_wgpu::Renderer as EguiRenderer;
use wgpu::util::DeviceExt;

/// Thin helper that constructs the egui renderer and provides texture/buffer helpers
pub struct Gui {
    pub renderer: EguiRenderer,
}

impl Gui {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let rdr = EguiRenderer::new(device, surface_format, None, 1);
        Self { renderer: rdr }
    }

    /// Upload textures referenced in delta and update buffers; returns a command encoder (not submitted)
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        primitives: &Vec<egui::ClippedPrimitive>,
        delta: &egui::TexturesDelta,
        config: &wgpu::SurfaceConfiguration,
        ctx: &egui::Context,
    ) -> wgpu::CommandEncoder {
        for (id, image_delta) in &delta.set {
            self.renderer.update_texture(device, queue, *id, image_delta);
        }

        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [config.width, config.height],
            pixels_per_point: ctx.pixels_per_point(),
        };

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Gui Encoder") });
        self.renderer.update_buffers(device, queue, &mut encoder, primitives, &screen_descriptor);

        encoder
    }
}
