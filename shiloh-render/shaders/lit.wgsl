// Lit forward pass — Blinn-Phong with directional + ambient light.
// Vertex: pos(3) normal(3) color(3)  — tightly packed f32 × 9

struct CameraUniform {
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    light_dir: vec4<f32>,
    light_color: vec4<f32>,
    ambient: vec4<f32>,
    time: vec4<f32>,
}

@group(0) @binding(0) var<uniform> camera: CameraUniform;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) model_col0: vec4<f32>,
    @location(4) model_col1: vec4<f32>,
    @location(5) model_col2: vec4<f32>,
    @location(6) model_col3: vec4<f32>,
}

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) world_n: vec3<f32>,
    @location(2) color: vec3<f32>,
}

@vertex
fn vs_main(in: VsIn) -> VsOut {
    let model = mat4x4<f32>(in.model_col0, in.model_col1, in.model_col2, in.model_col3);
    let world = model * vec4<f32>(in.position, 1.0);
    var out: VsOut;
    out.clip_pos = camera.view_proj * world;
    out.world_pos = world.xyz;
    // Assume uniform scale for normals (demo meshes).
    out.world_n = normalize((model * vec4<f32>(in.normal, 0.0)).xyz);
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let n = normalize(in.world_n);
    let l = normalize(-camera.light_dir.xyz);
    let v = normalize(camera.camera_pos.xyz - in.world_pos);
    let h = normalize(l + v);

    let ndotl = max(dot(n, l), 0.0);
    let spec = pow(max(dot(n, h), 0.0), 48.0);

    let albedo = in.color;
    let ambient = camera.ambient.xyz * albedo;
    let diffuse = camera.light_color.xyz * albedo * ndotl;
    let specular = camera.light_color.xyz * spec * 0.35;

    // Subtle time pulse on rim for demo flair.
    let rim = pow(1.0 - max(dot(n, v), 0.0), 3.0) * (0.15 + 0.1 * sin(camera.time.x * 2.0));
    let rgb = ambient + diffuse + specular + rim * camera.light_color.xyz;
    return vec4<f32>(rgb, 1.0);
}
