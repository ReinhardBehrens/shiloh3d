// HDR tonemap + color grade

struct PostUniform {
    params: vec4<f32>, // exposure, contrast, saturation, unused
}

@group(0) @binding(0) var<uniform> post: PostUniform;
@group(0) @binding(1) var src_tex: texture_2d<f32>;
@group(0) @binding(2) var src_samp: sampler;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
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
    out.clip = vec4<f32>(pos[idx], 0.0, 1.0);
    out.uv = pos[idx] * 0.5 + vec2<f32>(0.5, 0.5);
    out.uv.y = 1.0 - out.uv.y;
    return out;
}

fn aces(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return saturate((x * (a * x + b)) / (x * (c * x + d) + e));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    var rgb = textureSample(src_tex, src_samp, in.uv).rgb;
    rgb *= post.params.x;
    rgb = aces(rgb);
    let luma = dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
    rgb = mix(vec3<f32>(luma), rgb, post.params.z);
    rgb = (rgb - 0.5) * post.params.y + 0.5;
    // Slight cool grade for swamp mood.
    rgb *= vec3<f32>(0.92, 0.96, 1.05);
    return vec4<f32>(rgb, 1.0);
}
