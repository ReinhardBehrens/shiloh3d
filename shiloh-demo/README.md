# Shiloh3D Showcase Demo

Cross-platform demo for **Windows**, **macOS**, and **Linux**.

This binary currently drives the **wgpu extension** path to prove the frame loop quickly.  
Shiloh3D’s **primary** graphics direction is **native** (Vulkan / D3D12 / Metal); Web uses **WebGL** (+ WebGPU via wgpu). See [docs/GRAPHICS.md](../docs/GRAPHICS.md).

## Run

```bash
# Interactive (GPU + window)
cargo run -p shiloh-demo --release

# More instances (GPU instancing stress)
cargo run -p shiloh-demo --release -- --cubes 256

# CI / no display
cargo run -p shiloh-demo -- --headless-frames 120
```

## Controls

| Input | Action |
|---|---|
| **WASD** | Orbit / dolly |
| **Shift** | Boost camera move |
| **LMB drag** | Orbit |
| **Scroll** | Zoom |
| **R** | Reset camera |
| **Esc** | Quit |

## What it showcases

| System | In the demo |
|---|---|
| **shiloh-render** | Sky + grid + lit WGSL, depth buffer, GPU instancing |
| **shiloh-scene** | `Camera` view/projection |
| **shiloh-input** | Keys, mouse, action map |
| **shiloh-core** | Time, jobs, config, logging |
| **shiloh-ecs** | World spawn / components |
| **shiloh-physics** | Fixed-step stub world |
| **shiloh-animation** | Clip + blend tree + state machine |
| **shiloh-audio** | Mixer + spatial source |
| **shiloh-assets** | File load + package JSON |
| **shiloh-network** | In-memory replication packets |
| **shiloh-scripting** | Rust `ScriptModule` heartbeat |
| **shiloh-editor** | Project manifest on disk |
| **shiloh-rhi** | Null device + wgpu extension (native/WebGL stubs available on features) |

## Shaders

Bundled WGSL (compiled into `shiloh-render`):

- `shaders/sky.wgsl` — fullscreen procedural sky  
- `shaders/grid.wgsl` — world-space ground grid (brand crimson accents)  
- `shaders/lit.wgsl` — instanced Blinn-Phong mesh pass  

## Platform notes

Demo bring-up uses `winit` + **wgpu extension**:

- **Windows** — Win32 + DX12/Vulkan via wgpu  
- **macOS** — Cocoa + Metal via wgpu  
- **Linux** — X11/Wayland + Vulkan via wgpu  

Production titles will prefer the **native** RHI backend. Browsers target **WebGL** / WebGPU. Requires a GPU driver and (for windowed mode) a display server.
