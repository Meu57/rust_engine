// crates/engine_core/src/renderer/sprite_pass.rs

use std::num::NonZeroU64;
use wgpu::util::{DeviceExt, StagingBelt};
use engine_ecs::World;
use engine_shared::{CTransform, CSprite, CCamera};
use glam::{Mat4, Vec3};

use super::context::GraphicsContext;
use super::resources::RenderResources;
use super::types::{CameraUniform, InstanceRaw};
use super::frame_graph::{FrameInputs, PassDesc, PassKind, PhysicalResources, RenderPassNode};

// 100k sprites buffer (Audio Fix for Stutter)
const MAX_SPRITES: usize = 100_000;

pub struct SpritePass {
    render_pipeline: wgpu::RenderPipeline,
    instance_buffer: wgpu::Buffer,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    staging_belt: StagingBelt,
}

impl SpritePass {
    pub fn new(ctx: &GraphicsContext, resources: &RenderResources) -> Self {
        let camera_uniform = CameraUniform::default();
        let camera_buffer = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Camera Bind Group"),
            layout: &resources.camera_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: camera_buffer.as_entire_binding() }],
        });

        ctx.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let shader = ctx.device.create_shader_module(wgpu::include_wgsl!("../../../../assets/shaders/sprite.wgsl"));
        
        let render_pipeline_layout = ctx.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Sprite Pipeline Layout"),
            bind_group_layouts: &[&resources.camera_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = ctx.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Sprite Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: "vs_main", buffers: &[InstanceRaw::desc()] },
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
        let _ = pollster::block_on(ctx.device.pop_error_scope());

        // [AUDIO FIX] Persistent Buffer (Stops Reallocation Stutter)
        let instance_buffer_size = (MAX_SPRITES * std::mem::size_of::<InstanceRaw>()) as wgpu::BufferAddress;
        let instance_buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Instance Buffer (Persistent)"),
            size: instance_buffer_size,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let staging_belt = StagingBelt::new(1024);

        Self { render_pipeline, instance_buffer, camera_buffer, camera_bind_group, staging_belt }
    }

    pub fn draw(
        &mut self,
        ctx: &GraphicsContext,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        world: &World,
    ) {
        // [AUDIO FIX] Recycle memory (Stops "Sticky Fluid" memory leak)
        self.staging_belt.recall();

        // [VISUAL FIX] FORCE LOGICAL RESOLUTION
        // We ignore the physical window size (ctx.config.width) and force 
        // the camera projection to match the Game Logic (1280x720).
        // This ensures the "Invisible Wall" matches the edge of the screen exactly.
        let width = 1280.0;
        let height = 720.0;

        // --- CAMERA UPDATE ---
        let mut view_pos = Vec3::ZERO;
        let mut zoom = 1.0;

        if let (Some(cameras), Some(transforms)) = (world.query::<CCamera>(), world.query::<CTransform>()) {
            for (entity, cam_data) in cameras.iter() {
                if let Some(transform) = transforms.get(*entity) {
                    view_pos = Vec3::new(transform.pos.x, transform.pos.y, 0.0);
                    zoom = cam_data.zoom;
                    break;
                }
            }
        }

        let half_w = (width / 2.0) / zoom;
        let half_h = (height / 2.0) / zoom;

        let projection = Mat4::orthographic_rh(-half_w, half_w, -half_h, half_h, -100.0, 100.0);
        let view_matrix = Mat4::from_translation(-view_pos);
        let camera_data = CameraUniform {
            view_proj: (projection * view_matrix).to_cols_array_2d(),
        };

        ctx.queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[camera_data]));

        // --- INSTANCE COLLECTION ---
        let mut instances = Vec::new();
        if let (Some(transforms), Some(sprites)) = (world.query::<CTransform>(), world.query::<CSprite>()) {
            for (entity, transform) in transforms.iter() {
                if let Some(sprite) = sprites.get(*entity) {
                    if instances.len() >= MAX_SPRITES { break; }
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
        let current_data_size = instance_bytes.len() as wgpu::BufferAddress;

        if current_data_size > 0 {
            let mut buffer_view = self.staging_belt.write_buffer(
                encoder,
                &self.instance_buffer,
                0,
                NonZeroU64::new(current_data_size).unwrap(),
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
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.1, g: 0.2, b: 0.3, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.instance_buffer.slice(0..current_data_size));
            render_pass.draw(0..4, 0..instances.len() as u32);
        }
    }

    pub fn cleanup(&mut self) {
        self.staging_belt.recall();
    }
}

impl RenderPassNode for SpritePass {
    fn kind(&self) -> PassKind { PassKind::Sprite }
    fn execute<'a>(&mut self, ctx: &'a GraphicsContext, encoder: &mut wgpu::CommandEncoder, resources: &PhysicalResources<'a>, inputs: &FrameInputs<'a>, pass_desc: &PassDesc, _pass_index: usize) {
        encoder.push_debug_group(pass_desc.name);
        self.draw(ctx, encoder, resources.scene_color_view, inputs.world);
        encoder.pop_debug_group();
    }
}