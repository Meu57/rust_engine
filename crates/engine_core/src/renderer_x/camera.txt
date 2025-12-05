// crates/engine_core/src/renderer/camera.rs
use glam::Mat4;
use wgpu::util::DeviceExt;
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
}

pub struct Camera {
    pub buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    pub bind_group_layout: wgpu::BindGroupLayout,
}

impl Camera {
    /// Create camera buffer + bind group (bind group index 0)
    pub fn new(device: &wgpu::Device) -> Self {
        let camera_uniform = CameraUniform { view_proj: Mat4::IDENTITY.to_cols_array_2d() };
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: Some("camera_bind_group_layout"),
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
            label: Some("camera_bind_group"),
        });

        Self { buffer, bind_group, bind_group_layout }
    }

    /// Update camera with orthographic projection for pixel coordinates (0..width,0..height)
    pub fn update_orthographic(&self, queue: &wgpu::Queue, width: f32, height: f32) {
        let proj = Mat4::orthographic_rh(0.0, width, 0.0, height, -1.0, 1.0);
        let cam = CameraUniform { view_proj: proj.to_cols_array_2d() };
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&[cam]));
    }
}
