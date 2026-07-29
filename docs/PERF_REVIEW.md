# Shiloh3D — High-Performance Code Review

Review of hot paths in the foundation + showcase demo (`shiloh-demo`, `shiloh-render` GPU, `shiloh-core` jobs, `shiloh-ecs`).  
Scope: **correctness for real-time**, **cache behavior**, **GPU upload cost**, **threading**.  
Date: 2026-07-29 · Engine `0.1.0`

---

## Verdict

The architecture picks the right industry patterns (generational IDs, archetype SoA direction, jobs, instancing, render graph).  
Several hot paths are still **scaffold-grade** and will dominate frame time once content scales — fix before treating this as a shipping renderer.

| Area | Grade | Note |
|---|---|---|
| GPU instancing path | **B+** | Good: one draw per mesh type; contiguous `InstanceRaw` upload |
| CPU matrix build | **A-** | Rayon over reused `Vec<Mat4>` (no per-frame alloc) |
| Job system | **C+** | Works; busy-wait + yield is not a production scheduler |
| ECS structural moves | **C** | Insert/move path is incomplete; not yet SoA-tight |
| Uniform updates | **B** | Single camera UBO write/frame — good |
| Shader math | **B** | Cheap Blinn-Phong; rim uses `sin` — OK for demo |
| Frame allocator | **B** | Safe bump; unused in demo hot path yet |

---

## What’s working well

### 1. GPU instancing (`ForwardRenderer::render`)

- Cubes/spheres share one vertex/index buffer each; transforms go through an **instance buffer**.
- Draw calls scale as **O(mesh types)**, not O(objects) — correct for thousands of cubes.
- Instance buffer grows by powers of two — avoids realloc thrash when `--cubes` rises.

### 2. Contiguous POD uploads

- `Vertex` / `InstanceRaw` / `CameraUniform` are `#[repr(C)]` + `bytemuck::Pod`.
- `queue.write_buffer` + `cast_slice` — no per-instance GPU API spam.

### 3. Parallel CPU transforms

- Demo fills `Vec<Mat4>` with `rayon::par_iter_mut` — good for 256–4k instances.
- Engine `JobSystem` is also exercised (fence per frame).

### 4. Generational handles

- Index + generation packing prevents classic use-after-free bugs without GC.

---

## Issues (ordered by impact)

### P0 — Job system busy-waits

`JobHandle::wait` spins with `thread::yield_now()`. Under load this burns cores and fights the OS scheduler.

**Fix:** park/unpark or a futex/condvar; workers sleep on empty deques with a short backoff then park.

### P0 — ECS archetype moves are incomplete

`World::insert` / `swap_remove_row` do not fully relocate typed columns. Systems that assume dense SoA will see stale or empty columns as soon as gameplay mutates structure.

**Fix:** implement typed column `swap_remove` + full row migrate (or adopt a mature ECS like `hecs`/`bevy_ecs` temporarily).

### P1 — ~~Per-frame `Vec` allocation in the demo~~ (fixed)

Instance matrices now `resize` into `DemoApp::cube_mats` and refill in place.
### P1 — `write_buffer` of all instances every frame

Full instance buffer rewrite is fine to ~a few thousand mats; past that prefer:

- persistent mapping / staging ring, or  
- compute shader animation, or  
- only upload dirty ranges.

### P1 — Render graph execute closures are empty in the high-level `Renderer`

The showcase uses `ForwardRenderer` directly — good. The graph-based `Renderer` still runs no-op passes. Don’t double-pay for both paths.

### P2 — Grid shader `discard`

`discard` disables early-Z for that pass (already depth write off). Prefer alpha blend only, or a stencil mask, if fill-rate bound.

### P2 — Uniform scale normal transform

Lit shader assumes uniform scale (`model * normal`). Non-uniform scale will light incorrectly — use inverse-transpose when scales diverge.

### P2 — `present_mode: AutoVsync`

Fine for demo; for latency testing expose `Mailbox` / `Immediate` behind a CLI flag.

### P3 — Logging in script heartbeat

`debug!` every 120 frames is fine; avoid `info!` in per-frame systems.

---

## Hot-path checklist (before content scales)

- [ ] Reuse CPU instance buffers (no per-frame alloc)
- [ ] Job wait → proper sleep/notify
- [ ] Finish ECS structural moves + query iteration
- [ ] Depth pre-pass or sorted opaque draws when overdraw rises
- [ ] Bindless / material texture arrays (when assets land)
- [ ] Frustum cull before instance pack
- [ ] GPU timestamps / Tracy for real frame budgets
- [ ] Avoid `String` in components on the hot set (names → handles)

---

## Demo-specific notes

| Path | Cost model |
|---|---|
| Sky draw | 3 verts — trivial |
| Grid draw | 6 verts + fullscreen-ish FS — watch `fwidth` on large quads |
| Lit cubes | `draw_indexed` × instances — scales with `--cubes` |
| Showcase tick | Physics stub + audio silence fill + net every 30f — noise-level |

Headless mode (`--headless-frames`) skips GPU — use it in CI so platform matrix doesn’t need a display.

---

## Cross-platform GPU notes

| OS | Backend (wgpu default) | Review note |
|---|---|---|
| Windows | DX12 / Vulkan | Prefer DX12 on consumer; test both |
| macOS | Metal | `MemoryHints::Performance` OK; watch MoltenVK if Vulkan forced |
| Linux | Vulkan | Need `libvulkan` + X11/Wayland; validate on both WMs |

Surface resize must call `configure` (done). Handle `SurfaceError::Outdated` on Wayland compositors (done).

---

## Recommended next perf milestones

1. **Ring-buffered instance uploads** + no frame allocs  
2. **Real job sleep** + work stealing metrics  
3. **Complete archetype mover** + parallel queries with archetypal borrow rules  
4. **GPU-driven** animation for the cube field (compute)  
5. **Tracy / `wgpu` timestamps** in the demo HUD  

---

*This review is living documentation — update when the ECS mover and job wait land.*
