// assets/shaders/sprite.wgsl

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

// Camera uniform (group 0 binding 0)
struct CameraUniform {
    view_proj: mat4x4<f32>,
};
@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct InstanceInput {
    @location(0) model_matrix_0: vec4<f32>,
    @location(1) model_matrix_1: vec4<f32>,
    @location(2) model_matrix_2: vec4<f32>,
    @location(3) model_matrix_3: vec4<f32>,
    @location(4) color: vec4<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
    instance: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;

    // Standard quad in model space centered at origin (triangle strip)
    var pos = array<vec2<f32>, 4>(
        vec2<f32>(-0.5, 0.5),  // Top Left
        vec2<f32>(-0.5, -0.5), // Bottom Left
        vec2<f32>(0.5, 0.5),   // Top Right
        vec2<f32>(0.5, -0.5)   // Bottom Right
    );

    let vertex_pos = pos[in_vertex_index];

    // Reconstruct model matrix from columns
    let model_matrix = mat4x4<f32>(
        instance.model_matrix_0,
        instance.model_matrix_1,
        instance.model_matrix_2,
        instance.model_matrix_3,
    );

    let world_position = model_matrix * vec4<f32>(vertex_pos, 0.0, 1.0);

    // Multiply with camera view_proj to go to clip space
    out.clip_position = camera.view_proj * world_position;
    out.color = instance.color;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
