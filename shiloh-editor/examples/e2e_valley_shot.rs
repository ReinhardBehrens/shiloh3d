//! E2E: render a mockup-density Forest Valley frame to PNG for visual QA.

use std::path::PathBuf;

use glam::{Mat4, Quat, Vec3};
use shiloh_render::{SliceDrawParams, SliceRenderer, orthographic_light_matrix};
use shiloh_scene::Camera;

fn main() -> anyhow::Result<()> {
    let out = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("docs/screenshots/editor-forest-valley-e2e.png"));
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut renderer = pollster::block_on(SliceRenderer::new_offscreen(1280, 720))?;
    let mut cam = Camera::isometric(Vec3::new(0.0, 0.4, 1.5), 26.0);
    cam.fov_y_radians = 42f32.to_radians();
    cam.aspect = 1280.0 / 720.0;

    let mut foliage = Vec::new();
    let mut rocks = Vec::new();
    let mut mountains = Vec::new();
    let ground = [Mat4::from_scale_rotation_translation(
        Vec3::new(48.0, 0.12, 36.0),
        Quat::IDENTITY,
        Vec3::new(0.0, -0.05, 0.0),
    )];

    // Left ridge pines
    for i in 0..20 {
        let x = -12.0 + (i % 7) as f32 * 1.5;
        let z = -2.0 + (i / 7) as f32 * 2.4 + (i % 3) as f32 * 0.3;
        let h = 2.6 + (i % 4) as f32 * 0.35;
        foliage.push(Mat4::from_scale_rotation_translation(
            Vec3::new(1.1, h, 1.1),
            Quat::from_rotation_y(i as f32 * 0.35),
            Vec3::new(x, 0.0, z),
        ));
    }
    // Right ridge
    for i in 0..18 {
        let x = 3.5 + (i % 6) as f32 * 1.55;
        let z = -3.0 + (i / 6) as f32 * 2.2;
        let h = 2.3 + (i % 5) as f32 * 0.3;
        foliage.push(Mat4::from_scale_rotation_translation(
            Vec3::new(1.0, h, 1.0),
            Quat::from_rotation_y(i as f32 * 0.41),
            Vec3::new(x, 0.0, z),
        ));
    }
    // Near-water birches
    for i in 0..10 {
        foliage.push(Mat4::from_scale_rotation_translation(
            Vec3::new(0.75, 2.0, 0.75),
            Quat::IDENTITY,
            Vec3::new(-3.0 + i as f32 * 0.85, 0.0, 3.2 + (i % 2) as f32 * 0.6),
        ));
    }
    // Shore rocks — hero rock near center (gizmo target in mockup)
    rocks.push(Mat4::from_scale_rotation_translation(
        Vec3::new(2.4, 1.5, 2.1),
        Quat::from_rotation_y(0.6),
        Vec3::new(1.2, 0.55, 2.8),
    ));
    for i in 0..10 {
        rocks.push(Mat4::from_scale_rotation_translation(
            Vec3::new(1.0, 0.65, 0.9) * (0.7 + (i % 4) as f32 * 0.12),
            Quat::from_rotation_y(i as f32 * 0.5),
            Vec3::new(-2.0 + i as f32 * 0.75, 0.3, 1.6 + (i % 3) as f32 * 0.25),
        ));
    }
    // Cliffs
    rocks.push(Mat4::from_scale_rotation_translation(
        Vec3::new(4.0, 6.5, 1.6),
        Quat::IDENTITY,
        Vec3::new(-16.0, 2.8, 0.0),
    ));
    rocks.push(Mat4::from_scale_rotation_translation(
        Vec3::new(3.6, 5.5, 1.8),
        Quat::IDENTITY,
        Vec3::new(15.5, 2.4, -1.0),
    ));
    // Distant mountains (keep closer / lower fog so they read)
    for (i, (x, z, sx, sy)) in [
        (-20.0_f32, -16.0, 12.0, 10.0),
        (-6.0, -20.0, 16.0, 14.0),
        (8.0, -18.0, 14.0, 12.0),
        (22.0, -15.0, 13.0, 11.0),
        (0.0, -24.0, 20.0, 16.0),
    ]
    .iter()
    .enumerate()
    {
        mountains.push(Mat4::from_scale_rotation_translation(
            Vec3::new(*sx, *sy, *sx * 0.55),
            Quat::from_rotation_y(i as f32 * 0.15),
            Vec3::new(*x, sy * 0.4, *z),
        ));
    }

    let sun_dir = Vec3::new(-0.55, -0.85, -0.25).normalize();
    renderer.render(SliceDrawParams {
        view_proj: cam.view_proj(),
        camera_pos: cam.eye,
        time: 2.0,
        sun_dir,
        sun_color: Vec3::new(1.0, 0.94, 0.82),
        ambient: Vec3::new(0.10, 0.12, 0.13),
        fog_color: Vec3::new(0.62, 0.70, 0.76),
        fog_density: 0.008,
        light_view_proj: orthographic_light_matrix(sun_dir, Vec3::new(0.0, 0.5, 0.0), 45.0, 1.0, 100.0),
        point0_pos: Vec3::new(4.0, 3.0, 2.0),
        point0_range: 16.0,
        point0_color: Vec3::new(0.45, 0.65, 1.0) * 0.8,
        point1_pos: Vec3::new(-5.0, 2.2, -1.0),
        point1_range: 14.0,
        point1_color: Vec3::new(1.0, 0.5, 0.3) * 0.7,
        spot_pos: Vec3::new(0.0, 8.0, 0.0),
        spot_range: 20.0,
        spot_dir: Vec3::new(0.05, -1.0, 0.1).normalize(),
        spot_inner_cos: 0.9,
        spot_outer_cos: 0.7,
        spot_color: Vec3::new(1.0, 0.9, 0.65) * 1.5,
        exposure: 0.95,
        contrast: 1.12,
        saturation: 1.25,
        cube_instances: &[],
        sphere_instances: &[],
        extra_instances: &[],
        foliage_instances: &foliage,
        rock_instances: &rocks,
        mountain_instances: &mountains,
        ground_instances: &ground,
        prop0_instances: &[],
        prop1_instances: &[],
        prop2_instances: &[],
        prop3_instances: &[],
        skinned_model: None,
        skin_joints: &[],
        hud_verts: &[],
        draw_water: true,
        screenshot_path: None,
    })?;

    let (w, h, rgba) = renderer.read_rgba8()?;
    image::save_buffer(&out, &rgba, w, h, image::ColorType::Rgba8)?;
    println!(
        "Wrote {} ({w}x{h}, foliage={}, rocks={}, mountains={})",
        out.display(),
        foliage.len(),
        rocks.len(),
        mountains.len()
    );
    Ok(())
}
