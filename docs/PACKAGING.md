# Packaging — one-click Windows · macOS · Ubuntu

Shiloh ships **three consumer desktop programs** from one pack action:

| OS | Triple (x86_64 / arm) | Output folder |
|---|---|---|
| **Windows** | `x86_64-pc-windows-msvc` | `dist/desktop/windows-x86_64/` |
| **macOS** | `x86_64-apple-darwin` / `aarch64-apple-darwin` (CI) | `dist/desktop/macos-*` |
| **Ubuntu** | `x86_64-unknown-linux-gnu` | `dist/desktop/ubuntu-x86_64/` |

Binaries included by default: `shiloh-demo` (game/runtime showcase) and `shiloh-editor` (Studio).

## One click

```bash
# From repo root — builds host OS, stubs the other two locally
./packaging/one-click-pack.sh
# or
cargo run -p shiloh-cli -- pack
```

Studio: **Build → Pack Desktop** runs the same command.

Full three-OS artifacts (true Windows + Mac + Ubuntu binaries) come from GitHub Actions:

- Workflow: [`.github/workflows/pack-desktop.yml`](../.github/workflows/pack-desktop.yml)
- Trigger: **Actions → pack-desktop → Run workflow**, or push a `v*` tag

## Taskbar / Start bar icon

Same mark as in-app branding (`logo_shiloh3d.png`):

```bash
./packaging/install-icons.sh   # Linux .desktop + hicolor icons
```

Windows shortcuts can use `packaging/icons/shiloh3d.ico`. Runtime windows set the icon via `shiloh_app::window_icon()` / egui `with_icon`.

## OSS references (ideas we adapted — own code)

| Project | What we took | Where in Shiloh |
|---|---|---|
| [cargo-dist](https://github.com/axodotdev/cargo-dist) (Apache-2.0/MIT) | Multi-OS CI matrix + zip-per-platform layout | `.github/workflows/pack-desktop.yml`, `shiloh-cli` `pack` |
| [winit](https://github.com/rust-windowing/winit) (Apache-2.0/MIT) | Window / taskbar icon + Wayland app id / X11 class | `shiloh-app/src/icon.rs`, `windowed.rs` |
| Godot export presets | One click → platform packages UX | `packaging/one-click-pack.sh`, Studio Build menu |

We **do not** vendor cargo-dist; Shiloh owns the packer so titles stay **Christian-owned / bundleable**.

## Cook vs pack

| Command | Purpose |
|---|---|
| `shiloh-cli cook` | Asset package (`package.json` + data) for a project |
| `shiloh-cli pack` | **OS binaries** for Windows / macOS / Ubuntu |
