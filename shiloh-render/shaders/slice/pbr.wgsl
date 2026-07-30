// Slice PBR forward — multi-light, shadow, fog. Vertex: pos nrm uv color

struct FrameUniform {
    view_proj: mat4x4<f32>,
    light_view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    sun_dir: vec4<f32>,
    sun_color: vec4<f32>,
    ambient: vec4<f32>,
    fog: vec4<f32>,          // rgb + density
    point0_pos_range: vec4<f32>,
    point0_color: vec4<f32>,
    point1_pos_range: vec4<f32>,
    point1_color: vec4<f32>,
    spot_pos_range: vec4<f32>,
    spot_dir_cos: vec4<f32>,
    spot_color: vec4<f32>,
    params: vec4<f32>,       // x=time, y=exposure, z=grade_contrast, w=grade_sat
}

@group(0) @binding(0) var<uniform> frame: FrameUniform;
@group(0) @binding(1) var shadow_tex: texture_depth_2d;
@group(0) @binding(2) var shadow_samp: sampler_comparison;
@group(0) @binding(3) var albedo_tex: texture_2d<f32>;
@group(0) @binding(4) var albedo_samp: sampler;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec3<f32>,
    @location(4) model_col0: vec4<f32>,
    @location(5) model_col1: vec4<f32>,
    @location(6) model_col2: vec4<f32>,
    @location(7) model_col3: vec4<f32>,
}

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) world_n: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec3<f32>,
    @location(4) shadow_coord: vec4<f32>,
}

@vertex
fn vs_main(in: VsIn) -> VsOut {
    let model = mat4x4<f32>(in.model_col0, in.model_col1, in.model_col2, in.model_col3);
    let world = model * vec4<f32>(in.position, 1.0);
    var out: VsOut;
    out.clip = frame.view_proj * world;
    out.world_pos = world.xyz;
    out.world_n = normalize((model * vec4<f32>(in.normal, 0.0)).xyz);
    out.uv = in.uv;
    out.color = in.color;
    out.shadow_coord = frame.light_view_proj * world;
    return out;
}

fn shadow_factor(shadow_coord: vec4<f32>) -> f32 {
    var c = shadow_coord;
    c = c / c.w;
    let uv = c.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 || c.z > 1.0) {
        return 1.0;
    }
    let depth = c.z - 0.002;
    return textureSampleCompareLevel(shadow_tex, shadow_samp, uv, depth);
}

fn point_light(world: vec3<f32>, n: vec3<f32>, albedo: vec3<f32>, pos_range: vec4<f32>, col: vec4<f32>) -> vec3<f32> {
    let to_l = pos_range.xyz - world;
    let dist = length(to_l);
    let range = max(pos_range.w, 0.01);
    let atten = saturate(1.0 - dist / range);
    let atten2 = atten * atten;
    let l = normalize(to_l);
    let ndotl = max(dot(n, l), 0.0);
    return albedo * col.xyz * ndotl * atten2;
}

fn spot_light(
    world: vec3<f32>,
    n: vec3<f32>,
    albedo: vec3<f32>,
    pos_range: vec4<f32>,
    dir_cos: vec4<f32>,
    col: vec4<f32>,
) -> vec3<f32> {
    let to_l = pos_range.xyz - world;
    let dist = length(to_l);
    let range = max(pos_range.w, 0.01);
    let atten = saturate(1.0 - dist / range);
    let atten2 = atten * atten;
    let l = normalize(to_l);
    let spot_dir = normalize(dir_cos.xyz);
    let cos_outer = dir_cos.w;
    let cos_inner = col.w;
    let cos_angle = dot(-l, spot_dir);
    let cone = saturate((cos_angle - cos_outer) / max(cos_inner - cos_outer, 1e-4));
    let ndotl = max(dot(n, l), 0.0);
    return albedo * col.xyz * ndotl * atten2 * cone * cone;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let n = normalize(in.world_n);
    let tex = textureSample(albedo_tex, albedo_samp, in.uv).rgb;
    let albedo = tex * in.color;

    let l = normalize(-frame.sun_dir.xyz);
    let v = normalize(frame.camera_pos.xyz - in.world_pos);
    let h = normalize(l + v);
    let ndotl = max(dot(n, l), 0.0);
    let spec = pow(max(dot(n, h), 0.0), 64.0) * 0.25;
    let sh = shadow_factor(in.shadow_coord);

    var rgb = frame.ambient.xyz * albedo;
    rgb += frame.sun_color.xyz * albedo * ndotl * sh;
    rgb += frame.sun_color.xyz * spec * sh;
    rgb += point_light(in.world_pos, n, albedo, frame.point0_pos_range, frame.point0_color);
    rgb += point_light(in.world_pos, n, albedo, frame.point1_pos_range, frame.point1_color);
    rgb += spot_light(
        in.world_pos,
        n,
        albedo,
        frame.spot_pos_range,
        frame.spot_dir_cos,
        frame.spot_color,
    );

    let dist = distance(frame.camera_pos.xyz, in.world_pos);
    let fog_f = 1.0 - exp(-frame.fog.w * dist);
    rgb = mix(rgb, frame.fog.xyz, saturate(fog_f));

    return vec4<f32>(rgb, 1.0);
}
