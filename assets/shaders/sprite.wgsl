// assets/shaders/sprite.wgsl

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

// Data sent from the CPU (Our ECS Components)
struct InstanceInput {
    @location(0) model_matrix_0: vec4<f32>, // Transform Matrix Col 0
    @location(1) model_matrix_1: vec4<f32>, // Transform Matrix Col 1
    @location(2) model_matrix_2: vec4<f32>, // Transform Matrix Col 2
    @location(3) model_matrix_3: vec4<f32>, // Transform Matrix Col 3
    @location(4) color: vec4<f32>,          // Sprite Color
};

@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
    instance: InstanceInput,
) -> VertexOutput {
    var out: VertexOutput;

    // Hardcoded quad vertices (Triangle Strip)
    // 0--2
    // | /|
    // 1--3
    var pos = array<vec2<f32>, 4>(
        vec2<f32>(-0.5, 0.5),  // Top Left
        vec2<f32>(-0.5, -0.5), // Bottom Left
        vec2<f32>(0.5, 0.5),   // Top Right
        vec2<f32>(0.5, -0.5)   // Bottom Right
    );

    let vertex_pos = pos[in_vertex_index];

    // Reconstruct the matrix from the instance columns
    let model_matrix = mat4x4<f32>(
        instance.model_matrix_0,
        instance.model_matrix_1,
        instance.model_matrix_2,
        instance.model_matrix_3,
    );

    // Apply the transform (Model Space -> Clip Space)
    // Note: We are cheating and skipping View/Projection matrices for 5 minutes.
    // We are drawing directly in Normalized Device Coordinates (NDC).
    // X and Y must be between -1.0 and 1.0 to be visible.
    
    // Divide by 500.0 to scale our "World Pixels" down to NDC for testing
    let pixel_pos = model_matrix * vec4<f32>(vertex_pos, 0.0, 1.0);
    
    out.clip_position = vec4<f32>(pixel_pos.x / 500.0, pixel_pos.y / 500.0, 0.0, 1.0);
    out.color = instance.color;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}