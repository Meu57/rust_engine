// crates/engine_core/src/renderer/mod.rs
pub mod instance;
pub mod camera;
pub mod pipeline;
pub mod extractor;
pub mod gui;

use winit::window::Window;
use wgpu::util::DeviceExt;
use engine_ecs::World;
use crate::renderer::{
    instance::InstanceRaw,
    camera::Camera,
    pipeline::create_render_pipeline,
    extractor::extract_instances,
    gui::Gui,
};

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,
    render_pipeline: wgpu::RenderPipeline,
    instance_buffer: wgpu::Buffer,

    camera: Camera,
    pub gui: Gui,
}

impl Renderer {
    pub async fn new(window: &Window) -> Self {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let surface = unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::from_window(&window).unwrap())
        }.unwrap();

        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }).await.expect("Failed to find an appropriate adapter");

        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                label: None,
            },
            None,
        ).await.unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps.formats.iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &config);

        // camera + gui
        let camera = Camera::new(&device);
        let gui = Gui::new(&device, surface_format);

        // pipeline
        let render_pipeline = create_render_pipeline(&device, config.format, &camera.bind_group_layout);

        // instance buffer initial
        let instance_data = vec![InstanceRaw { model: [[0.0; 4]; 4], color: [0.0; 4] }; 100];
        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Instance Buffer"),
            contents: bytemuck::cast_slice(&instance_data),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        Self {
            surface,
            device,
            queue,
            config,
            size,
            render_pipeline,
            instance_buffer,
            camera,
            gui,
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    pub fn render(
        &mut self,
        world: &World,
        gui_ctx: Option<(&egui::Context, &Vec<egui::ClippedPrimitive>, &egui::TexturesDelta)>
    ) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        // update camera
        let width = self.config.width as f32;
        let height = self.config.height as f32;
        self.camera.update_orthographic(&self.queue, width, height);

        // extract instances
        let instances = extract_instances(world);

        // upload or resize instance buffer
        let instance_bytes = bytemuck::cast_slice(&instances);
        if (instance_bytes.len() as wgpu::BufferAddress) > self.instance_buffer.size() {
            self.instance_buffer.destroy();
            self.instance_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Instance Buffer"),
                contents: instance_bytes,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            });
        } else {
            self.queue.write_buffer(&self.instance_buffer, 0, instance_bytes);
        }

        // record game render pass
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.1, g: 0.2, b: 0.3, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.camera.bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.instance_buffer.slice(0..((instances.len() * std::mem::size_of::<InstanceRaw>()) as u64)));
            render_pass.draw(0..4, 0..instances.len() as u32);
        }

        // submit game pass
        self.queue.submit(std::iter::once(encoder.finish()));

        // gui overlay
        if let Some((ctx, primitives, delta)) = gui_ctx {
            let mut gui_encoder = self.gui.prepare(&self.device, &self.queue, primitives, delta, &self.config, ctx);
            {
                let mut gui_pass = gui_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Gui Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                self.gui.renderer.render(&mut gui_pass, primitives, &egui_wgpu::ScreenDescriptor {
                    size_in_pixels: [self.config.width, self.config.height],
                    pixels_per_point: ctx.pixels_per_point(),
                });
            }

            for id in &delta.free {
                self.gui.renderer.free_texture(id);
            }

            self.queue.submit(std::iter::once(gui_encoder.finish()));
        }

        output.present();
        Ok(())
    }
}
