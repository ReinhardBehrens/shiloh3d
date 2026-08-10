//! One-click desktop pack — Windows · macOS · Ubuntu binaries.
//!
//! # Design (own code, OSS references)
//! - **Release matrix idea** inspired by [cargo-dist](https://github.com/axodotdev/cargo-dist)
//!   (Apache-2.0 / MIT): native runners per OS rather than fragile cross from one host.
//! - **Godot-like one export click**: Studio / CLI invoke a single `pack` that always
//!   builds the host binary and stages a `dist/desktop/` layout; CI fills the other two OS.
//! - **Bevy / wgpu desktop targets**: same triples the Rust gamedev ecosystem ships
//!   (`x86_64-pc-windows-msvc`, `*-apple-darwin`, `x86_64-unknown-linux-gnu`).
//!
//! We do **not** vendor cargo-dist; Shiloh owns the packer so games stay bundleable
//! without a store lock-in.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

/// Triple shipped for the three consumer desktop OSes.
#[derive(Debug, Clone, Copy)]
pub struct DesktopTarget {
    pub id: &'static str,
    pub triple: &'static str,
    pub folder: &'static str,
    pub exe_name: &'static str,
}

/// Windows · macOS · Ubuntu (x86_64) — the required ship set.
pub const DESKTOP_TARGETS: &[DesktopTarget] = &[
    DesktopTarget {
        id: "windows",
        triple: "x86_64-pc-windows-msvc",
        folder: "windows-x86_64",
        exe_name: "shiloh-demo.exe",
    },
    DesktopTarget {
        id: "macos",
        triple: "x86_64-apple-darwin",
        folder: "macos-x86_64",
        exe_name: "shiloh-demo",
    },
    DesktopTarget {
        id: "ubuntu",
        triple: "x86_64-unknown-linux-gnu",
        folder: "ubuntu-x86_64",
        exe_name: "shiloh-demo",
    },
];

/// Which binaries to release-pack by default (game + studio).
pub const DEFAULT_BINS: &[&str] = &["shiloh-demo", "shiloh-editor", "lions-gate"];

#[derive(Debug, Clone)]
pub struct PackOptions {
    pub workspace: PathBuf,
    pub out_dir: PathBuf,
    pub bins: Vec<String>,
    /// Build only the host triple locally; leave stubs for others (CI fills them).
    pub host_only: bool,
    pub release: bool,
}

impl Default for PackOptions {
    fn default() -> Self {
        Self {
            workspace: PathBuf::from("."),
            out_dir: PathBuf::from("dist/desktop"),
            bins: DEFAULT_BINS.iter().map(|s| (*s).to_string()).collect(),
            host_only: true,
            release: true,
        }
    }
}

fn host_triple_fallback() -> &'static str {
    // Compile-time guess when `rustc -vV` is unavailable.
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        "x86_64-unknown-linux-gnu"
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        "x86_64-apple-darwin"
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        "aarch64-apple-darwin"
    }
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        "x86_64-pc-windows-msvc"
    }
    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "windows", target_arch = "x86_64"),
    )))]
    {
        "unknown"
    }
}

/// Detect host triple at runtime via `rustc -vV` (more accurate than compile cfg alone).
pub fn detect_host_triple() -> Result<String> {
    let out = Command::new("rustc")
        .args(["-vV"])
        .output()
        .context("run rustc -vV")?;
    if !out.status.success() {
        bail!("rustc -vV failed");
    }
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("host: ") {
            return Ok(rest.trim().to_string());
        }
    }
    bail!("could not parse rustc host triple")
}

fn exe_name_for(bin: &str, triple: &str) -> String {
    if triple.contains("windows") {
        format!("{bin}.exe")
    } else {
        bin.to_string()
    }
}

fn cargo_bin_path(workspace: &Path, triple: &str, release: bool, bin: &str) -> PathBuf {
    let profile = if release { "release" } else { "debug" };
    let name = exe_name_for(bin, triple);
    // When building for host without --target, cargo puts bins in target/<profile>/.
    // Cross/host-with-target uses target/<triple>/<profile>/.
    let with_triple = workspace.join("target").join(triple).join(profile).join(&name);
    if with_triple.is_file() {
        return with_triple;
    }
    workspace.join("target").join(profile).join(name)
}

fn write_readme(out: &Path, built: &[(String, String)], skipped: &[(String, String)]) -> Result<()> {
    let mut md = String::from(
        "# Shiloh3D desktop pack\n\n\
         One-click layout: **Windows · macOS · Ubuntu** game/editor binaries.\n\n\
         ## References (ideas, not copied source)\n\
         - [cargo-dist](https://github.com/axodotdev/cargo-dist) — multi-OS release matrix\n\
         - [winit](https://github.com/rust-windowing/winit) — window / taskbar icon APIs\n\
         - Godot export presets — one click → platform packages\n\n\
         ## Built this run\n\n",
    );
    for (t, note) in built {
        md.push_str(&format!("- **{t}**: {note}\n"));
    }
    if !skipped.is_empty() {
        md.push_str("\n## Not built on this host (CI / native runner)\n\n");
        for (t, note) in skipped {
            md.push_str(&format!("- **{t}**: {note}\n"));
        }
        md.push_str(
            "\nPush a tag or run `.github/workflows/pack-desktop.yml` to fill Windows + macOS + Ubuntu artifacts.\n",
        );
    }
    md.push_str("\nTaskbar icon: `packaging/icons/shiloh3d.png` / `shiloh3d.ico` (same as `logo_shiloh3d.png`).\n");
    fs::write(out.join("README.md"), md)?;
    Ok(())
}

/// Pack desktop binaries into `out_dir/{windows,macos,ubuntu}-x86_64/`.
pub fn pack_desktop(opts: &PackOptions) -> Result<PathBuf> {
    let host = detect_host_triple().unwrap_or_else(|_| host_triple_fallback().to_string());
    fs::create_dir_all(&opts.out_dir).context("create dist/desktop")?;

    let mut built = Vec::new();
    let mut skipped = Vec::new();

    for target in DESKTOP_TARGETS {
        let dest = opts.out_dir.join(target.folder);
        fs::create_dir_all(&dest)?;

        let can_native = host == target.triple
            || (host.starts_with("x86_64-unknown-linux") && target.id == "ubuntu")
            || (host.contains("apple-darwin") && target.id == "macos")
            || (host.contains("windows") && target.id == "windows");

        if opts.host_only && !can_native {
            let stub = dest.join("NOT_BUILT_ON_THIS_HOST.txt");
            fs::write(
                &stub,
                format!(
                    "Target {} ({}) was not built on host {host}.\n\
                     Build on a {} runner or via GitHub Actions pack-desktop.yml.\n",
                    target.id, target.triple, target.id
                ),
            )?;
            skipped.push((
                target.id.to_string(),
                format!("{} — stub only on this host", target.triple),
            ));
            continue;
        }

        // Build each requested bin for this triple.
        for bin in &opts.bins {
            let mut cmd = Command::new("cargo");
            cmd.current_dir(&opts.workspace)
                .arg("build")
                .arg("-p")
                .arg(bin)
                .arg("--bin")
                .arg(bin);
            if bin == "shiloh-editor" {
                // Editor UI binary is feature-gated.
                cmd.args(["--features", "ui"]);
            }
            if opts.release {
                cmd.arg("--release");
            }
            // Always pass --target so output path is stable across hosts.
            cmd.arg("--target").arg(target.triple);
            // Ensure target is installed when possible.
            let _ = Command::new("rustup")
                .args(["target", "add", target.triple])
                .status();

            let status = cmd
                .status()
                .with_context(|| format!("cargo build {bin} --target {}", target.triple))?;
            if !status.success() {
                if opts.host_only {
                    fs::write(
                        dest.join("BUILD_FAILED.txt"),
                        format!("cargo build failed for {} / {bin}\n", target.triple),
                    )?;
                    skipped.push((
                        target.id.to_string(),
                        format!("build failed for {bin} ({})", target.triple),
                    ));
                    continue;
                }
                bail!("cargo build failed for {} / {bin}", target.triple);
            }

            let src = cargo_bin_path(&opts.workspace, target.triple, opts.release, bin);
            if !src.is_file() {
                // Host build without nested triple dir fallback.
                let alt = opts
                    .workspace
                    .join("target")
                    .join(if opts.release { "release" } else { "debug" })
                    .join(exe_name_for(bin, target.triple));
                if alt.is_file() {
                    let dest_bin = dest.join(exe_name_for(bin, target.triple));
                    fs::copy(&alt, &dest_bin)
                        .with_context(|| format!("copy {} → {}", alt.display(), dest_bin.display()))?;
                    built.push((
                        target.id.to_string(),
                        format!("{bin} → {}", dest_bin.display()),
                    ));
                    continue;
                }
                bail!("built binary missing: {}", src.display());
            }
            let dest_bin = dest.join(exe_name_for(bin, target.triple));
            fs::copy(&src, &dest_bin)
                .with_context(|| format!("copy {} → {}", src.display(), dest_bin.display()))?;
            built.push((
                target.id.to_string(),
                format!("{bin} → {}", dest_bin.display()),
            ));
        }

        // Stage icons next to binaries (Windows shortcuts / Linux desktop).
        let icon_png = opts.workspace.join("packaging/icons/shiloh3d.png");
        let icon_ico = opts.workspace.join("packaging/icons/shiloh3d.ico");
        if icon_png.is_file() {
            let _ = fs::copy(&icon_png, dest.join("shiloh3d.png"));
        }
        if icon_ico.is_file() {
            let _ = fs::copy(&icon_ico, dest.join("shiloh3d.ico"));
        }
    }

    write_readme(&opts.out_dir, &built, &skipped)?;
    Ok(opts.out_dir.clone())
}
