// crates/engine_core/src/renderer/resources.rs
use std::num::NonZeroU64;

use wgpu;

use crate::renderer::types::CameraUniform;

/// Centralized GPU resource definitions shared across passes.
///
/// Phase 2+ goal: this becomes the single source of truth for
/// bind group layouts (camera, globals, materials, shadows, etc.).
pub struct RenderResources {
    pub camera_layout: wgpu::BindGroupLayout,
    // Future:
    // pub global_layout: wgpu::BindGroupLayout,
    // pub material_layout: wgpu::BindGroupLayout,
    // pub shadow_layout: wgpu::BindGroupLayout,
}

impl RenderResources {
    pub fn new(device: &wgpu::Device) -> Self {
        // Minimum size for our camera uniform buffer.
        let min_size = NonZeroU64::new(std::mem::size_of::<CameraUniform>() as u64);

        let camera_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Camera BindGroupLayout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    // Camera is typically used in both vertex and fragment stages.
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: min_size,
                    },
                    count: None,
                }],
            });

        Self { camera_layout }
    }
}
