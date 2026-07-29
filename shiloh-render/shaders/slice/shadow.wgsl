// Directional shadow depth pass

struct ShadowUniform {
    light_view_proj: mat4x4<f32>,
}

@group(0) @binding(0) var<uniform> shadow: ShadowUniform;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(4) model_col0: vec4<f32>,
    @location(5) model_col1: vec4<f32>,
    @location(6) model_col2: vec4<f32>,
    @location(7) model_col3: vec4<f32>,
}

@vertex
fn vs_main(in: VsIn) -> @builtin(position) vec4<f32> {
    let model = mat4x4<f32>(in.model_col0, in.model_col1, in.model_col2, in.model_col3);
    let world = model * vec4<f32>(in.position, 1.0);
    return shadow.light_view_proj * world;
}
