// crates/engine_core/src/renderer/instance.rs
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct InstanceRaw {
    pub model: [[f32; 4]; 4],
    pub color: [f32; 4],
}

impl InstanceRaw {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<InstanceRaw>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // model matrix (4 columns)
                wgpu::VertexAttribute { offset: 0, shader_location: 0, format: wgpu::VertexFormat::Float32x4 },
                wgpu::VertexAttribute { offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress, shader_location: 1, format: wgpu::VertexFormat::Float32x4 },
                wgpu::VertexAttribute { offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress, shader_location: 2, format: wgpu::VertexFormat::Float32x4 },
                wgpu::VertexAttribute { offset: mem::size_of::<[f32; 12]>() as wgpu::BufferAddress, shader_location: 3, format: wgpu::VertexFormat::Float32x4 },
                // color
                wgpu::VertexAttribute { offset: mem::size_of::<[f32; 16]>() as wgpu::BufferAddress, shader_location: 4, format: wgpu::VertexFormat::Float32x4 },
            ],
        }
    }
}
