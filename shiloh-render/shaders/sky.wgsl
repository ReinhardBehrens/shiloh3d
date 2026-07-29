// Fullscreen sky gradient (procedural, no textures).

struct CameraUniform {
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    light_dir: vec4<f32>,
    light_color: vec4<f32>,
    ambient: vec4<f32>,
    time: vec4<f32>,
}

@group(0) @binding(0) var<uniform> camera: CameraUniform;

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VsOut {
    var pos = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0),
    );
    var out: VsOut;
    out.clip_pos = vec4<f32>(pos[idx], 0.999, 1.0);
    out.uv = pos[idx] * 0.5 + vec2<f32>(0.5, 0.5);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let t = in.uv.y;
    let top = vec3<f32>(0.12, 0.16, 0.28);
    let horizon = vec3<f32>(0.55, 0.35, 0.32);
    let bottom = vec3<f32>(0.05, 0.05, 0.07);
    let sky = mix(horizon, top, smoothstep(0.45, 1.0, t));
    let ground = mix(bottom, horizon, smoothstep(0.0, 0.45, t));
    let rgb = mix(ground, sky, step(0.45, t));
    // Soft sun disc in light direction projected naively on UV.
    let sun_uv = vec2<f32>(0.72, 0.62);
    let sun = exp(-40.0 * dot(in.uv - sun_uv, in.uv - sun_uv));
    let color = rgb + camera.light_color.xyz * sun * 0.9;
    return vec4<f32>(color, 1.0);
}
