// Water v1 — depth tint + animated normals (screen-space-ish fresnel)

struct FrameUniform {
    view_proj: mat4x4<f32>,
    light_view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    sun_dir: vec4<f32>,
    sun_color: vec4<f32>,
    ambient: vec4<f32>,
    fog: vec4<f32>,
    point0_pos_range: vec4<f32>,
    point0_color: vec4<f32>,
    point1_pos_range: vec4<f32>,
    point1_color: vec4<f32>,
    spot_pos_range: vec4<f32>,
    spot_dir_cos: vec4<f32>,
    spot_color: vec4<f32>,
    params: vec4<f32>,
}

@group(0) @binding(0) var<uniform> frame: FrameUniform;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world_xz: vec2<f32>,
    @location(1) world_pos: vec3<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VsOut {
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(-40.0, -40.0),
        vec2<f32>( 40.0, -40.0),
        vec2<f32>( 40.0,  40.0),
        vec2<f32>(-40.0, -40.0),
        vec2<f32>( 40.0,  40.0),
        vec2<f32>(-40.0,  40.0),
    );
    let p = positions[idx];
    var out: VsOut;
    let world = vec3<f32>(p.x, 0.05, p.y);
    out.clip = frame.view_proj * vec4<f32>(world, 1.0);
    out.world_xz = p;
    out.world_pos = world;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let t = frame.params.x;
    let n = normalize(vec3<f32>(
        sin(in.world_xz.x * 0.35 + t * 1.5) * 0.15,
        1.0,
        cos(in.world_xz.y * 0.35 + t * 1.2) * 0.15,
    ));
    let v = normalize(frame.camera_pos.xyz - in.world_pos);
    let fresnel = pow(1.0 - max(dot(n, v), 0.0), 3.0);
    let deep = vec3<f32>(0.05, 0.12, 0.14);
    let shallow = vec3<f32>(0.12, 0.28, 0.26);
    let sun = max(dot(n, normalize(-frame.sun_dir.xyz)), 0.0);
    var rgb = mix(deep, shallow, sun * 0.5 + 0.3);
    rgb += frame.sun_color.xyz * fresnel * 0.35;
    let dist = distance(frame.camera_pos.xyz, in.world_pos);
    let fog_f = 1.0 - exp(-frame.fog.w * dist);
    rgb = mix(rgb, frame.fog.xyz, saturate(fog_f));
    let alpha = 0.65 + fresnel * 0.25;
    return vec4<f32>(rgb, alpha);
}
