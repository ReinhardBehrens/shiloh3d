// Infinite-style ground grid in XZ, world-space fragment shading.

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
    @location(0) world_xz: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VsOut {
    // Large quad on XZ plane.
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(-80.0, -80.0),
        vec2<f32>( 80.0, -80.0),
        vec2<f32>( 80.0,  80.0),
        vec2<f32>(-80.0, -80.0),
        vec2<f32>( 80.0,  80.0),
        vec2<f32>(-80.0,  80.0),
    );
    let p = positions[idx];
    var out: VsOut;
    let world = vec4<f32>(p.x, 0.0, p.y, 1.0);
    out.clip_pos = camera.view_proj * world;
    out.world_xz = p;
    return out;
}

fn grid_line(coord: f32, width: f32) -> f32 {
    let g = abs(fract(coord - 0.5) - 0.5) / max(fwidth(coord), 1e-4);
    return 1.0 - min(g / width, 1.0);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let xz = in.world_xz;
    let minor = max(grid_line(xz.x, 1.2), grid_line(xz.y, 1.2));
    let major = max(grid_line(xz.x / 5.0, 1.5), grid_line(xz.y / 5.0, 1.5));
    let line = max(minor * 0.35, major * 0.85);

    let dist = length(xz - camera.camera_pos.xz);
    let fade = 1.0 - smoothstep(40.0, 75.0, dist);
    let base = vec3<f32>(0.08, 0.09, 0.11);
    let accent = vec3<f32>(0.75, 0.12, 0.22); // brand crimson
    let color = mix(base, accent, line * 0.65);
    let alpha = line * fade * 0.85;
    if (alpha < 0.02) {
        discard;
    }
    return vec4<f32>(color, alpha);
}
