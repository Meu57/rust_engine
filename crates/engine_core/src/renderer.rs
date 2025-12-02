#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct InstanceRaw {
    model: [[f32; 4]; 4], // Mat4 transform
    color: [f32; 4],      // Vec4 color
}

impl InstanceRaw {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<InstanceRaw>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance, // Update per INSTANCE (entity), not per vertex
            attributes: &[
                // Mat4 takes 4 vec4 slots (Locations 0, 1, 2, 3)
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 12]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // Color (Location 4)
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 16]>() as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

use winit::window::Window;
use wgpu::util::DeviceExt;
use engine_ecs::World;
use engine_shared::{CTransform, CSprite};
use glam::Mat4;

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,
    render_pipeline: wgpu::RenderPipeline,
    instance_buffer: wgpu::Buffer,
}

impl Renderer {
    // We need the window because the 'Surface' lives on it.
    pub async fn new(window: &Window) -> Self {
        let size = window.inner_size();

        // 1. Instance: The handle to our GPU (Vulkan/DX12)
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY, 
            ..Default::default()
        });

        // 2. Surface: The part of the window we draw to.
        // UNSAFE: We must ensure the window lives as long as the surface. 
        // Since 'App' owns both, this is safe in our architecture.
        let surface = unsafe { 
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::from_window(&window).unwrap()) 
        }.unwrap();

        // 3. Adapter: The physical graphics card handle.
        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }).await.expect("Failed to find an appropriate adapter");

        // 4. Device & Queue: The logical connection to the GPU.
        // Device = Creates resources (textures, buffers).
        // Queue = Sends commands (draw calls) to the GPU.
        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                label: None,
            },
            None,
        ).await.unwrap();

        // 5. Config: How the swapchain works (VSync, Format, etc.)
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
            present_mode: wgpu::PresentMode::Fifo, // Fifo = VSync On (Cap at 60 FPS)
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &config);

        // --- PIPELINE CREATION ---
        
        // 1. Load Shader
        let shader = device.create_shader_module(wgpu::include_wgsl!("../../../assets/shaders/sprite.wgsl"));

        // 2. Create Pipeline Layout
        let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[], // No textures yet
            push_constant_ranges: &[],
        });

        // 3. Create the Pipeline
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[InstanceRaw::desc()], // <--- Tell it about our instance data
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip, // We draw quads as strips
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // Draw both sides
                ..Default::default()
            },
            depth_stencil: None, // No depth testing for 2D yet
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        // 4. Create a dummy instance buffer (capacity for 100 sprites)
        // We will overwrite this every frame.
        let instance_data = vec![InstanceRaw { model: [[0.0;4];4], color: [0.0;4] }; 100];
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

    pub fn render(&mut self, world: &World) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        // 1. EXTRACT DATA FROM ECS
        // Query (Transform, Sprite)
        let mut instances = Vec::new();
        if let (Some(transforms), Some(sprites)) = (world.query::<CTransform>(), world.query::<CSprite>()) {
            // Zip the iterators (Entity match)
            for (entity, transform) in transforms.iter() {
                if let Some(sprite) = sprites.get(*entity) {
                    // Convert Transform to Matrix
                    let model = Mat4::from_scale_rotation_translation(
                        glam::Vec3::new(transform.scale.x * 50.0, transform.scale.y * 50.0, 1.0), // Scale up so we can see it
                        glam::Quat::from_rotation_z(transform.rotation),
                        glam::Vec3::new(transform.pos.x, transform.pos.y, 0.0),
                    );

                    instances.push(InstanceRaw {
                        model: model.to_cols_array_2d(),
                        color: sprite.color.to_array(),
                    });
                }
            }
        }

        // 2. UPLOAD TO GPU
        // Write the instance data to the buffer
        self.queue.write_buffer(
            &self.instance_buffer,
            0,
            bytemuck::cast_slice(&instances),
        );

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
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1, g: 0.2, b: 0.3, a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            // 3. DRAW
            render_pass.set_pipeline(&self.render_pipeline);
            // Slot 0 is the Instance Data
            render_pass.set_vertex_buffer(0, self.instance_buffer.slice(0..((instances.len() * std::mem::size_of::<InstanceRaw>()) as u64)));
            // Draw 4 vertices (Quad), N instances
            render_pass.draw(0..4, 0..instances.len() as u32);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}