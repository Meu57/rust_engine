// crates/engine_core/src/renderer/sprite_pass.rs

use std::num::NonZeroU64;
use wgpu::util::{DeviceExt, StagingBelt};
use engine_ecs::World;
use engine_shared::{CTransform, CSprite, CCamera}; // Added CCamera
use glam::{Mat4, Vec3};

use super::context::GraphicsContext;
use super::resources::RenderResources;
use super::types::{CameraUniform, InstanceRaw};
use super::frame_graph::{FrameInputs, PassDesc, PassKind, PhysicalResources, RenderPassNode};

pub struct SpritePass {
    render_pipeline: wgpu::RenderPipeline,
    instance_buffer: wgpu::Buffer,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    staging_belt: StagingBelt,
}

impl SpritePass {
    pub fn new(ctx: &GraphicsContext, resources: &RenderResources) -> Self {
        // Camera buffer
        let camera_uniform = CameraUniform::default();
        let camera_buffer =
            ctx.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Camera Buffer"),
                    contents: bytemuck::cast_slice(&[camera_uniform]),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                });

        // Camera bind group uses the shared layout from RenderResources
        let camera_bind_group =
            ctx.device
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Camera Bind Group"),
                    layout: &resources.camera_layout,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: camera_buffer.as_entire_binding(),
                    }],
                });

        // ---------------------------------------------------------------------
        // Shader + pipeline creation with validation error scope
        // ---------------------------------------------------------------------
        ctx.device.push_error_scope(wgpu::ErrorFilter::Validation);

        let shader = ctx
            .device
            .create_shader_module(wgpu::include_wgsl!(
                "../../../../assets/shaders/sprite.wgsl"
            ));

        let render_pipeline_layout =
            ctx.device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("Sprite Pipeline Layout"),
                    // Use shared camera layout
                    bind_group_layouts: &[&resources.camera_layout],
                    push_constant_ranges: &[],
                });

        let render_pipeline =
            ctx.device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Sprite Render Pipeline"),
                    layout: Some(&render_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: "vs_main",
                        buffers: &[InstanceRaw::desc()],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: "fs_main",
                        targets: &[Some(wgpu::ColorTargetState {
                            format: ctx.config.format,
                            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleStrip,
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: None,
                        ..Default::default()
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    multiview: None,
                });

        let pipeline_error = pollster::block_on(ctx.device.pop_error_scope());
        if let Some(err) = pipeline_error {
            panic!("SpritePass pipeline creation failed validation: {:?}", err);
        }

        let instance_data = vec![
            InstanceRaw {
                model: [[0.0; 4]; 4],
                color: [0.0; 4],
            };
            100
        ];

        let instance_buffer =
            ctx.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Instance Buffer"),
                    contents: bytemuck::cast_slice(&instance_data),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });

        let staging_belt = StagingBelt::new(1024);

        Self {
            render_pipeline,
            instance_buffer,
            camera_buffer,
            camera_bind_group,
            staging_belt,
        }
    }

    pub fn draw(
        &mut self,
        ctx: &GraphicsContext,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        world: &World,
    ) {
        let width = ctx.config.width as f32;
        let height = ctx.config.height as f32;

        // --- NEW CAMERA LOGIC ---
        let mut view_pos = Vec3::ZERO;
        let mut zoom = 1.0;

        // Query for active camera
        if let (Some(cameras), Some(transforms)) = (world.query::<CCamera>(), world.query::<CTransform>()) {
            for (entity, cam_data) in cameras.iter() {
                if let Some(transform) = transforms.get(*entity) {
                    view_pos = Vec3::new(transform.pos.x, transform.pos.y, 0.0);
                    zoom = cam_data.zoom;
                    break;
                }
            }
        }

        // Projection (Zoom)
        let half_w = (width / 2.0) / zoom;
        let half_h = (height / 2.0) / zoom;

        let projection = Mat4::orthographic_rh(
            -half_w, half_w, 
            -half_h, half_h, 
            -100.0, 100.0
        );

        // View (Position)
        let view_matrix = Mat4::from_translation(-view_pos);

        let camera_data = CameraUniform {
            view_proj: (projection * view_matrix).to_cols_array_2d(),
        };
        // ------------------------

        ctx.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[camera_data]));

        let mut instances = Vec::new();
        if let (Some(transforms), Some(sprites)) =
            (world.query::<CTransform>(), world.query::<CSprite>())
        {
            for (entity, transform) in transforms.iter() {
                if let Some(sprite) = sprites.get(*entity) {
                    let model = Mat4::from_scale_rotation_translation(
                        Vec3::new(transform.scale.x * 50.0, transform.scale.y * 50.0, 1.0),
                        glam::Quat::from_rotation_z(transform.rotation),
                        Vec3::new(transform.pos.x, transform.pos.y, 0.0),
                    );

                    instances.push(InstanceRaw {
                        model: model.to_cols_array_2d(),
                        color: sprite.color.to_array(),
                    });
                }
            }
        }

        let instance_bytes = bytemuck::cast_slice(&instances);
        let required_size = instance_bytes.len() as wgpu::BufferAddress;

        if required_size > self.instance_buffer.size() {
            let old_size = self.instance_buffer.size().max(256);
            self.instance_buffer.destroy();

            let mut new_size = (required_size * 2).max(old_size);
            new_size = wgpu::util::align_to(new_size, 4);

            self.instance_buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Instance Buffer"),
                size: new_size,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        if required_size > 0 {
            let non_zero = NonZeroU64::new(required_size).unwrap();
            let mut buffer_view = self.staging_belt.write_buffer(
                encoder,
                &self.instance_buffer,
                0,
                non_zero,
                &ctx.device,
            );
            buffer_view.copy_from_slice(instance_bytes);
        }

        self.staging_belt.finish();

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Sprite Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);

            let slice_size =
                (instances.len() * std::mem::size_of::<InstanceRaw>()) as wgpu::BufferAddress;
            render_pass.set_vertex_buffer(0, self.instance_buffer.slice(0..slice_size));
            render_pass.draw(0..4, 0..instances.len() as u32);
        }
    }

    pub fn cleanup(&mut self) {
        self.staging_belt.recall();
    }
}

impl RenderPassNode for SpritePass {
    fn kind(&self) -> PassKind {
        PassKind::Sprite
    }

    fn execute<'a>(
        &mut self,
        ctx: &'a GraphicsContext,
        encoder: &mut wgpu::CommandEncoder,
        resources: &PhysicalResources<'a>,
        inputs: &FrameInputs<'a>,
        pass_desc: &PassDesc,
        _pass_index: usize,
    ) {
        encoder.push_debug_group(pass_desc.name);
        self.draw(ctx, encoder, resources.scene_color_view, inputs.world);
        encoder.pop_debug_group();
    }
}