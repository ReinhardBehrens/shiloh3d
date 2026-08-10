//! E2E visual gate — **pass only when Forest Valley matches the uploaded FirstGoal Studio look**.
//!
//! Reference (user-uploaded editor mockup, viewport crop):
//! `docs/references/firstgoal-valley-viewport.png`
//! Full chrome: `docs/references/firstgoal-studio-editor.png`
//!
//! ```bash
//! cargo run -p shiloh-editor --example visual_gate
//! ```
//! Exit **0** only if similarity + FirstGoal quality features + Phase requirements all pass.
//! Exit **1** otherwise (CI-friendly). Heuristic-only “green checklists” are not enough.

use std::path::{Path, PathBuf};

use glam::{Mat4, Quat, Vec3};
use shiloh_render::{SliceDrawParams, SliceRenderer, orthographic_light_matrix};
use shiloh_scene::Camera;

/// Composite similarity must clear this (calibrated: greybox proxies score ~0.10–0.20).
const SIM_PASS: f32 = 0.42;
/// Soft floor for FirstGoal feature checklist (0–100).
const FEATURE_PASS: u32 = 70;
/// Soft floor for Phase 5 / Compete requirements (0–100).
const REQ_PASS: u32 = 80;

fn main() -> anyhow::Result<()> {
    let png_out = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("docs/screenshots/gate-latest.png"));
    let report_out = std::env::args()
        .nth(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("docs/screenshots/gate-report.md"));
    let reference = std::env::args()
        .nth(3)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("docs/references/firstgoal-valley-viewport.png"));

    if let Some(parent) = png_out.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut renderer = pollster::block_on(SliceRenderer::new_offscreen(1280, 720))?;
    let mut cam = Camera::isometric(Vec3::new(0.0, 0.4, 1.5), 26.0);
    cam.fov_y_radians = 42f32.to_radians();
    cam.aspect = 1280.0 / 720.0;

    let (foliage, rocks, mountains, ground, props) = build_valley_instances();
    let sun_dir = Vec3::new(-0.55, -0.85, -0.25).normalize();
    renderer.render(SliceDrawParams {
        view_proj: cam.view_proj(),
        camera_pos: cam.eye,
        time: 2.0,
        sun_dir,
        sun_color: Vec3::new(1.45, 0.95, 0.55) * 1.85,
        ambient: Vec3::new(0.12, 0.10, 0.08),
        fog_color: Vec3::new(0.78, 0.62, 0.42),
        fog_density: 0.0025,
        light_view_proj: orthographic_light_matrix(
            sun_dir,
            Vec3::new(0.0, 0.5, 0.0),
            48.0,
            1.0,
            110.0,
        ),
        point0_pos: Vec3::new(6.0, 4.0, 3.0),
        point0_range: 22.0,
        point0_color: Vec3::new(0.45, 0.62, 1.0) * 0.7,
        point1_pos: Vec3::new(-7.0, 3.0, -2.0),
        point1_range: 18.0,
        point1_color: Vec3::new(1.15, 0.45, 0.22) * 0.95,
        spot_pos: Vec3::new(0.0, 10.0, 2.0),
        spot_range: 28.0,
        spot_dir: Vec3::new(0.1, -1.0, 0.15).normalize(),
        spot_inner_cos: 0.92,
        spot_outer_cos: 0.72,
        spot_color: Vec3::new(1.15, 0.92, 0.65) * 1.55,
        exposure: 1.22,
        contrast: 1.35,
        saturation: 1.22,
        cube_instances: &[],
        sphere_instances: &[],
        extra_instances: &[],
        foliage_instances: &foliage,
        rock_instances: &rocks,
        mountain_instances: &mountains,
        ground_instances: &ground,
        prop0_instances: &props.0,
        prop1_instances: &props.1,
        prop2_instances: &props.2,
        prop3_instances: &props.3,
        skinned_model: None,
        skin_joints: &[],
        hud_verts: &[],
        draw_water: true,
        screenshot_path: None,
    })?;

    let (w, h, rgba) = renderer.read_rgba8()?;
    image::save_buffer(&png_out, &rgba, w, h, image::ColorType::Rgba8)?;
    let _ = std::fs::copy(&png_out, "docs/screenshots/01-editor-viewport.png");

    let density = foliage.len()
        + rocks.len()
        + mountains.len()
        + props.0.len()
        + props.1.len()
        + props.2.len()
        + props.3.len();

    let sim = compare_to_reference(&rgba, w, h, &reference)?;
    let features = score_firstgoal_features(&rgba, w, h);
    let requirements = score_phase_requirements(density);

    let pass = sim.score >= SIM_PASS
        && features.total >= FEATURE_PASS
        && requirements.total >= REQ_PASS;

    let report = format_report(
        &png_out,
        &reference,
        w,
        h,
        density,
        &sim,
        &features,
        &requirements,
        pass,
    );
    std::fs::write(&report_out, &report)?;

    println!(
        "E2E FirstGoal gate: {} · sim={:.3} (need ≥ {SIM_PASS}) · features={}/100 · reqs={}/100",
        if pass { "PASS" } else { "FAIL" },
        sim.score,
        features.total,
        requirements.total,
    );
    for line in &sim.lines {
        println!("  {line}");
    }
    for line in &features.lines {
        println!("  {line}");
    }
    for line in &requirements.lines {
        println!("  {line}");
    }
    println!("  → {} · {}", png_out.display(), report_out.display());

    if pass {
        Ok(())
    } else {
        anyhow::bail!(
            "E2E gate FAILED vs FirstGoal Studio viewport — see {}",
            report_out.display()
        )
    }
}

struct SimReport {
    score: f32,
    lines: Vec<String>,
}

struct Checklist {
    total: u32,
    lines: Vec<String>,
}

fn compare_to_reference(
    rgba: &[u8],
    w: u32,
    h: u32,
    reference: &Path,
) -> anyhow::Result<SimReport> {
    if !reference.is_file() {
        anyhow::bail!("missing FirstGoal reference: {}", reference.display());
    }
    let ref_img = image::open(reference)?.to_rgba8();
    let tw = 320u32;
    let th = 180u32;
    let cand = image::imageops::resize(
        &image::RgbaImage::from_raw(w, h, rgba.to_vec())
            .ok_or_else(|| anyhow::anyhow!("bad capture buffer"))?,
        tw,
        th,
        image::imageops::FilterType::Triangle,
    );
    let refer = image::imageops::resize(&ref_img, tw, th, image::imageops::FilterType::Triangle);

    let mut sum_abs = 0f64;
    let mut hist_a = [0u64; 72];
    let mut hist_b = [0u64; 72];
    let mut la = Vec::with_capacity((tw * th) as usize);
    let mut lb = Vec::with_capacity((tw * th) as usize);
    let n = (tw * th) as f64;

    for (pa, pb) in cand.pixels().zip(refer.pixels()) {
        let a = pa.0;
        let b = pb.0;
        for i in 0..3 {
            sum_abs += (a[i] as f64 - b[i] as f64).abs();
            hist_a[i * 24 + (a[i] as usize * 24 / 256)] += 1;
            hist_b[i * 24 + (b[i] as usize * 24 / 256)] += 1;
        }
        la.push(0.2126 * a[0] as f32 + 0.7152 * a[1] as f32 + 0.0722 * a[2] as f32);
        lb.push(0.2126 * b[0] as f32 + 0.7152 * b[1] as f32 + 0.0722 * b[2] as f32);
    }

    let mae = (sum_abs / (n * 3.0) / 255.0) as f32;
    let hist_corr = corr_hist(&hist_a, &hist_b);
    let ssim = ssim_luma(&la, &lb);
    // Composite: SSIM dominates; hist helps color grade; MAE penalizes flat wrong colors.
    let score = (0.55 * ssim + 0.25 * hist_corr.max(0.0) + 0.20 * (1.0 - mae).clamp(0.0, 1.0))
        .clamp(0.0, 1.0);

    Ok(SimReport {
        score,
        lines: vec![
            format!(
                "sim composite={score:.3} (SSIM {ssim:.3} · hist_corr {hist_corr:.3} · mae {mae:.3})"
            ),
            format!("reference: {}", reference.display()),
        ],
    })
}

fn corr_hist(a: &[u64; 72], b: &[u64; 72]) -> f32 {
    let sa: f64 = a.iter().sum::<u64>() as f64;
    let sb: f64 = b.iter().sum::<u64>() as f64;
    if sa < 1.0 || sb < 1.0 {
        return 0.0;
    }
    let fa: Vec<f64> = a.iter().map(|&x| x as f64 / sa).collect();
    let fb: Vec<f64> = b.iter().map(|&x| x as f64 / sb).collect();
    let ma = fa.iter().sum::<f64>() / fa.len() as f64;
    let mb = fb.iter().sum::<f64>() / fb.len() as f64;
    let mut num = 0.0;
    let mut da = 0.0;
    let mut db = 0.0;
    for (x, y) in fa.iter().zip(fb.iter()) {
        num += (x - ma) * (y - mb);
        da += (x - ma) * (x - ma);
        db += (y - mb) * (y - mb);
    }
    if da < 1e-12 || db < 1e-12 {
        return 0.0;
    }
    (num / (da.sqrt() * db.sqrt())) as f32
}

fn ssim_luma(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len() as f32;
    if n < 1.0 {
        return 0.0;
    }
    let ma = a.iter().sum::<f32>() / n;
    let mb = b.iter().sum::<f32>() / n;
    let mut sa = 0.0;
    let mut sb = 0.0;
    let mut cov = 0.0;
    for (&x, &y) in a.iter().zip(b.iter()) {
        sa += (x - ma) * (x - ma);
        sb += (y - mb) * (y - mb);
        cov += (x - ma) * (y - mb);
    }
    sa /= n;
    sb /= n;
    cov /= n;
    let c1 = (0.01f32 * 255.0).powi(2);
    let c2 = (0.03f32 * 255.0).powi(2);
    ((2.0 * ma * mb + c1) * (2.0 * cov + c2)) / ((ma * ma + mb * mb + c1) * (sa + sb + c2))
}

/// FirstGoal viewport must read: sky, textured terrain, dense foliage, water, depth, sun.
fn score_firstgoal_features(rgba: &[u8], w: u32, h: u32) -> Checklist {
    let mut total = 0u32;
    let mut lines = Vec::new();
    let n = (w as usize) * (h as usize);
    if n == 0 || rgba.len() < n * 4 {
        lines.push("+0 empty frame".into());
        return Checklist { total: 0, lines };
    }

    // Upper band sky: cool / blue-ish (FirstGoal clear sky).
    let mut sky_cool = 0u64;
    let mut sky_n = 0u64;
    let y1 = (h as usize) / 3;
    for y in 0..y1 {
        for x in (0..w as usize).step_by(4) {
            let o = (y * w as usize + x) * 4;
            let r = rgba[o] as f32;
            let _g = rgba[o + 1] as f32;
            let b = rgba[o + 2] as f32;
            if b > r * 1.05 && b > 80.0 {
                sky_cool += 1;
            }
            sky_n += 1;
        }
    }
    let sky_ratio = sky_cool as f32 / sky_n.max(1) as f32;
    if sky_ratio > 0.12 {
        total += 20;
        lines.push(format!("+20 sky/cool upper band ({:.0}%)", sky_ratio * 100.0));
    } else {
        lines.push(format!(
            "+0  weak sky/cool upper band ({:.0}%) — FirstGoal has blue sky",
            sky_ratio * 100.0
        ));
    }

    // Edge density ≈ foliage / rock detail (not flat primitives).
    let mut edge = 0f32;
    let mut edge_n = 0f32;
    for y in (1..h as usize).step_by(2) {
        for x in (1..w as usize).step_by(2) {
            let o = (y * w as usize + x) * 4;
            let o_l = (y * w as usize + x - 1) * 4;
            let o_u = ((y - 1) * w as usize + x) * 4;
            let lum = |i: usize| {
                0.2126 * rgba[i] as f32 + 0.7152 * rgba[i + 1] as f32 + 0.0722 * rgba[i + 2] as f32
            };
            edge += (lum(o) - lum(o_l)).abs() + (lum(o) - lum(o_u)).abs();
            edge_n += 1.0;
        }
    }
    let edge_mean = edge / edge_n.max(1.0);
    // FirstGoal crop ~24 at 320px; gate greybox ~13. Need richer mid-freq detail.
    if edge_mean > 18.0 {
        total += 20;
        lines.push(format!("+20 mid-freq detail / edges ({edge_mean:.1})"));
    } else {
        lines.push(format!(
            "+0  low edge detail ({edge_mean:.1}) — proxies look flat vs FirstGoal pines"
        ));
    }

    // Large flat-color run penalty: photoreal breaks long runs.
    let mut long_runs = 0u64;
    let mut run = 1u32;
    let mut prev = (0u8, 0u8, 0u8);
    for i in (0..n).step_by(8) {
        let o = i * 4;
        let c = (rgba[o] / 16, rgba[o + 1] / 16, rgba[o + 2] / 16);
        if c == prev {
            run += 1;
            if run == 40 {
                long_runs += 1;
            }
        } else {
            prev = c;
            run = 1;
        }
    }
    if long_runs < 80 {
        total += 15;
        lines.push(format!("+15 not overly flat runs ({long_runs})"));
    } else {
        lines.push(format!(
            "+0  too many flat color runs ({long_runs}) — greybox plates"
        ));
    }

    // Water / river: cool midtones in lower-mid band with some variance.
    let mut cool = 0u64;
    let mut band_n = 0u64;
    let y0 = (h as usize) * 45 / 100;
    let y1 = (h as usize) * 75 / 100;
    for y in (y0..y1).step_by(2) {
        for x in (0..w as usize).step_by(4) {
            let o = (y * w as usize + x) * 4;
            let r = rgba[o] as f32;
            let b = rgba[o + 2] as f32;
            if b > r * 1.04 {
                cool += 1;
            }
            band_n += 1;
        }
    }
    let cool_ratio = cool as f32 / band_n.max(1) as f32;
    if cool_ratio > 0.08 {
        total += 15;
        lines.push(format!(
            "+15 water/cool mid band ({:.0}%)",
            cool_ratio * 100.0
        ));
    } else {
        lines.push(format!(
            "+0  missing river/cool mid band ({:.0}%)",
            cool_ratio * 100.0
        ));
    }

    // Depth: luminance spread.
    let mut lum_min = 255.0f32;
    let mut lum_max = 0.0f32;
    for i in (0..n).step_by(16) {
        let o = i * 4;
        let lum =
            0.2126 * rgba[o] as f32 + 0.7152 * rgba[o + 1] as f32 + 0.0722 * rgba[o + 2] as f32;
        lum_min = lum_min.min(lum);
        lum_max = lum_max.max(lum);
    }
    let spread = (lum_max - lum_min) / 255.0;
    if spread > 0.55 {
        total += 15;
        lines.push(format!("+15 depth luminance spread ({spread:.2})"));
    } else {
        lines.push(format!("+0  weak depth spread ({spread:.2})"));
    }

    // Warm sun key (FirstGoal golden key light).
    let mut warm = 0u64;
    let mut samples = 0u64;
    for i in (0..n).step_by(8) {
        let o = i * 4;
        let r = rgba[o] as f32;
        let b = rgba[o + 2] as f32;
        if r > b * 1.08 {
            warm += 1;
        }
        samples += 1;
    }
    let warm_ratio = warm as f32 / samples.max(1) as f32;
    if warm_ratio > 0.08 {
        total += 15;
        lines.push(format!("+15 warm sun key ({:.0}%)", warm_ratio * 100.0));
    } else {
        lines.push(format!(
            "+0  weak warm sun ({:.0}%)",
            warm_ratio * 100.0
        ));
    }

    Checklist { total, lines }
}

/// Latest Phase 5 / Compete requirements (authoring + gate plumbing).
fn score_phase_requirements(density: usize) -> Checklist {
    let mut total = 0u32;
    let mut lines = Vec::new();

    let checks: &[(&str, bool, u32)] = &[
        (
            "EDITOR_UX.md borrow map",
            Path::new("docs/EDITOR_UX.md").is_file(),
            10,
        ),
        (
            "FirstGoal reference present",
            Path::new("docs/references/firstgoal-valley-viewport.png").is_file()
                && Path::new("docs/references/firstgoal-studio-editor.png").is_file(),
            15,
        ),
        (
            "Landscape/Foliage scene types",
            Path::new("shiloh-scene/src/terrain.rs").is_file()
                && Path::new("shiloh-scene/src/foliage.rs").is_file(),
            10,
        ),
        (
            "Rhai host + ScriptComponent",
            Path::new("shiloh-scripting/src/rhai_host.rs").is_file()
                && Path::new("shiloh-scripting/src/script_component.rs").is_file(),
            10,
        ),
        (
            "Editor layouts + content cook stubs",
            Path::new("shiloh-editor/src/layouts.rs").is_file()
                && Path::new("shiloh-editor/src/asset_cook.rs").is_file(),
            10,
        ),
        (
            "RayAccurate / Parry crate",
            Path::new("shiloh-ray/src/lib.rs").is_file(),
            10,
        ),
        (
            "Blender peer pipeline doc",
            Path::new("docs/BLENDER_PIPELINE.md").is_file(),
            5,
        ),
        (
            "Phase Compete spec",
            Path::new("docs/PHASE_COMPETE.md").is_file(),
            5,
        ),
        (
            "Valley instance density ≥ 60",
            density >= 60,
            15,
        ),
        (
            "Gate still path writable",
            Path::new("docs/screenshots").is_dir(),
            10,
        ),
    ];

    for (label, ok, pts) in checks {
        if *ok {
            total += pts;
            lines.push(format!("+{pts} {label}"));
        } else {
            lines.push(format!("+0  missing: {label}"));
        }
    }

    Checklist { total, lines }
}

fn format_report(
    png: &Path,
    reference: &Path,
    w: u32,
    h: u32,
    density: usize,
    sim: &SimReport,
    features: &Checklist,
    requirements: &Checklist,
    pass: bool,
) -> String {
    let mut out = String::new();
    out.push_str("# Phase Compete — E2E FirstGoal gate report\n\n");
    out.push_str(&format!(
        "**Result:** {} · **Similarity:** {:.3} (need ≥ {SIM_PASS}) · **Features:** {}/100 · **Requirements:** {}/100\n\n",
        if pass { "PASS" } else { "FAIL" },
        sim.score,
        features.total,
        requirements.total
    ));
    out.push_str(&format!(
        "- Capture: `{}` ({w}×{h})\n",
        png.display()
    ));
    out.push_str(&format!(
        "- Reference (uploaded editor viewport): `{}`\n",
        reference.display()
    ));
    out.push_str(&format!("- Instance density: {density}\n"));
    out.push_str("- Spec: [PHASE_COMPETE.md](../PHASE_COMPETE.md) · mockup: [firstgoal-studio-editor.png](../references/firstgoal-studio-editor.png)\n\n");

    out.push_str("## Similarity vs FirstGoal viewport\n\n");
    for line in &sim.lines {
        out.push_str(&format!("- `{line}`\n"));
    }
    out.push_str("\n## FirstGoal quality features\n\n");
    for line in &features.lines {
        out.push_str(&format!("- `{line}`\n"));
    }
    out.push_str("\n## Latest Phase 5 / Compete requirements\n\n");
    for line in &requirements.lines {
        out.push_str(&format!("- `{line}`\n"));
    }
    out.push_str("\n## Goal\n\n");
    out.push_str(
        "E2E success = capture **matches** the uploaded FirstGoal Studio valley still (photoreal pines, river, sky, atmosphere) **and** Phase 5 authoring requirements. Greybox proxies must **fail**.\n",
    );
    out
}

fn build_valley_instances() -> (
    Vec<Mat4>,
    Vec<Mat4>,
    Vec<Mat4>,
    Vec<Mat4>,
    (Vec<Mat4>, Vec<Mat4>, Vec<Mat4>, Vec<Mat4>),
) {
    let mut foliage = Vec::new();
    let mut rocks = Vec::new();
    let mut mountains = Vec::new();
    let ground = vec![Mat4::from_scale_rotation_translation(
        Vec3::new(56.0, 0.12, 42.0),
        Quat::IDENTITY,
        Vec3::new(0.0, -0.06, 0.0),
    )];
    let mut prop0 = Vec::new();
    let mut prop1 = Vec::new();
    let mut prop2 = Vec::new();
    let mut prop3 = Vec::new();

    for i in 0..24 {
        let x = -14.0 + (i % 8) as f32 * 1.45;
        let z = -2.0 + (i / 8) as f32 * 2.2 + (i % 3) as f32 * 0.25;
        let h = 3.2 + (i % 5) as f32 * 0.35;
        foliage.push(Mat4::from_scale_rotation_translation(
            Vec3::new(1.2, h, 1.2),
            Quat::from_rotation_y(i as f32 * 0.33),
            Vec3::new(x, 0.0, z),
        ));
    }
    for i in 0..20 {
        let x = 3.0 + (i % 7) as f32 * 1.5;
        let z = -4.0 + (i / 7) as f32 * 2.1;
        foliage.push(Mat4::from_scale_rotation_translation(
            Vec3::new(1.05, 2.8 + (i % 4) as f32 * 0.3, 1.05),
            Quat::from_rotation_y(i as f32 * 0.4),
            Vec3::new(x, 0.0, z),
        ));
    }
    for i in 0..12 {
        foliage.push(Mat4::from_scale_rotation_translation(
            Vec3::new(0.8, 2.4, 0.8),
            Quat::IDENTITY,
            Vec3::new(-3.5 + i as f32 * 0.8, 0.0, 3.0 + (i % 2) as f32 * 0.5),
        ));
    }

    rocks.push(Mat4::from_scale_rotation_translation(
        Vec3::new(2.6, 1.6, 2.2),
        Quat::from_rotation_y(0.55),
        Vec3::new(1.0, 0.5, 2.6),
    ));
    for i in 0..12 {
        rocks.push(Mat4::from_scale_rotation_translation(
            Vec3::new(0.95, 0.7, 0.85) * (0.75 + (i % 4) as f32 * 0.1),
            Quat::from_rotation_y(i as f32 * 0.45),
            Vec3::new(-2.2 + i as f32 * 0.7, 0.28, 1.4 + (i % 3) as f32 * 0.2),
        ));
    }
    rocks.push(Mat4::from_scale_rotation_translation(
        Vec3::new(4.2, 7.0, 1.7),
        Quat::IDENTITY,
        Vec3::new(-17.0, 3.0, 0.0),
    ));
    rocks.push(Mat4::from_scale_rotation_translation(
        Vec3::new(3.8, 6.0, 1.9),
        Quat::IDENTITY,
        Vec3::new(16.0, 2.6, -1.2),
    ));

    for (i, (x, z, sx, sy)) in [
        (-22.0_f32, -16.0, 14.0, 11.0),
        (-6.0, -21.0, 18.0, 15.0),
        (10.0, -19.0, 15.0, 13.0),
        (24.0, -15.0, 14.0, 12.0),
        (0.0, -26.0, 22.0, 17.0),
    ]
    .iter()
    .enumerate()
    {
        mountains.push(Mat4::from_scale_rotation_translation(
            Vec3::new(*sx, *sy, *sx * 0.55),
            Quat::from_rotation_y(i as f32 * 0.12),
            Vec3::new(*x, sy * 0.38, *z),
        ));
    }

    for i in 0..6 {
        prop0.push(Mat4::from_translation(Vec3::new(
            -5.0 + i as f32,
            0.0,
            2.0 + (i % 2) as f32,
        )));
        prop1.push(Mat4::from_translation(Vec3::new(
            4.0 + i as f32 * 0.6,
            0.0,
            -2.0,
        )));
    }
    prop2.push(Mat4::from_scale_rotation_translation(
        Vec3::splat(1.4),
        Quat::IDENTITY,
        Vec3::new(0.5, 0.0, 1.5),
    ));
    prop3.push(Mat4::from_scale_rotation_translation(
        Vec3::splat(0.9),
        Quat::IDENTITY,
        Vec3::new(-1.0, 0.0, 2.0),
    ));

    (foliage, rocks, mountains, ground, (prop0, prop1, prop2, prop3))
}
