# Shiloh3D — Technology choices

**Principle:** prefer **custom Shiloh code**. Pull in third-party crates only when necessary.  
When you do, **wrap them behind stable Shiloh APIs** so physics, rendering, windowing, and audio can be replaced without rewriting games.

Do **not** permanently re-export every dependency through the public engine API.

---

## Recommended starting stack

| System | Starting point | Shiloh ownership |
|---|---|---|
| **Language** | Rust | — |
| **Rendering** | **wgpu** + **WGSL** (bring-up) | `shiloh-rhi` / `shiloh-render` — never require games to import `wgpu` |
| **Window / input** | **winit** | `shiloh-app` + `shiloh-input` — map to Shiloh events/keys |
| **Mathematics** | **glam** | Allowed in public math-heavy types *for now*; prefer `shiloh_math` façade if glam must be swapped |
| **ECS** | **Custom** (`shiloh-ecs`) | Own archetypes/queries; wrap an external ECS only if we abandon custom |
| **Editor GUI** | **egui** initially | `shiloh-editor` only — games/editor panels talk to Shiloh UI traits |
| **Physics** | **Rapier** initially | `shiloh-physics` (`PhysicsBackend`) — Rapier stays private |
| **Audio** | **Kira** or native lib | `shiloh-audio` — mixer/listener API is ours |
| **Assets** | **glTF** first | `shiloh-assets` importers; engine formats versioned |
| **Serialization** | **serde** + versioned engine formats | Public types use Shiloh schemas; serde is impl detail |
| **Parallel work** | **Rayon** initially → **custom jobs** | `shiloh-core::JobSystem` is the long-term API; Rayon OK inside impls |
| **Profiling** | **Tracy** + GPU timestamps | Engine hooks; Tracy not in game-facing API |
| **Scripting** | **Rust first**; visual / embedded later | `shiloh-scripting` |

---

## Custom-first vs dependency

| Prefer to write ourselves | OK to adopt (wrapped) |
|---|---|
| ECS, handles, jobs (long-term), scene formats, net replication model, editor UX | wgpu (start), winit, glam, Rapier, Kira/cpal, egui, serde, Tracy, glTF parsers |
| RHI trait surface, materials, render graph | Platform GPU drivers (via native or wgpu) |

If a dependency would appear in **every game’s `Cargo.toml`**, stop — put a Shiloh façade in front.

---

## Public API boundary rules

1. **Games depend on `shiloh-*` crates**, not on `wgpu`, `winit`, `rapier3d`, `kira`, `egui`, `tracy-client`.  
2. **Feature flags** may enable backends internally (`shiloh-rhi/wgpu`, future `shiloh-physics/rapier`).  
3. **Adapter modules** live under `*_backend` / `platform` and are `pub(crate)` or feature-gated `doc(hidden)` unless needed for advanced embedders.  
4. **Types that cross the API** (vectors, entities, asset IDs) should be Shiloh-owned or thin, documented exceptions (e.g. glam until a math façade exists).  
5. **Replacing a backend** must not break scene files or gameplay crates — only engine internals and Cargo features.

### Current leaks to close

| Leak | Where | Fix |
|---|---|---|
| `ForwardRenderer::new(Arc<winit::Window>)` | `shiloh-render` | Accept Shiloh surface / raw-window-handle wrapper from `shiloh-app` |
| Demo depends on `winit` / `wgpu` / `rayon` directly | `shiloh-demo` | Acceptable for samples; production games should use `shiloh-app` only |
| `glam` in public `Transform` / `Camera` | `shiloh-scene` | Documented exception; optional `shiloh-math` later |

Physics and audio already expose **Shiloh traits/types** with stub backends — keep Rapier/Kira behind `PhysicsBackend` / audio device traits.

---

## Rendering note (reconciles with native-first)

- **Starting point:** wgpu + WGSL behind `shiloh-rhi` / `shiloh-render` (fast, cross-platform, WebGPU-ready).  
- **Long-term shipping desktop:** native Vulkan / D3D12 / Metal backends implementing the **same** RHI traits ([GRAPHICS.md](GRAPHICS.md)).  
- **Web:** WebGL + WebGPU (wgpu); still no raw `wgpu` in game code.  

So: wgpu is the **advised bootstrap**, not a permanent public dependency.

---

## Parallelism note

- Use **Rayon** for data-parallel fills (e.g. instance matrices) inside engine/demo code.  
- Prefer **`shiloh-core::JobSystem`** for frame-structured work and as the stable API games call.  
- Grow the custom job system until Rayon is an optional impl detail, not the interface.

---

## Serialization / assets

- **glTF** first for interchange.  
- **serde** for JSON/TOML tooling and manifests.  
- Add **versioned binary/text engine formats** (`shiloh.scene`, materials, prefabs) with migration — do not freeze on raw glTF as the only runtime format.

---

## Editor

- **egui** for the first polished panels (hierarchy, inspector, viewport overlays).  
- All editor state goes through `shiloh-editor` / ECS / scene APIs so a future custom UI shell can drop egui.

---

## Scripting

1. Rust `ScriptModule` (now)  
2. Embedded language and/or visual graph (later)  
3. Never require game teams to link Boa/Rhai directly — only `shiloh-scripting`

---

## Profiling

- Integrate **Tracy** (CPU) and **GPU timestamps** in the render path.  
- Gate behind `profiling` feature; zero cost when disabled.

---

## Checklist for new dependencies

Before adding a crate to the workspace:

- [ ] Can we implement a thinner custom version in reasonable time?  
- [ ] Is it confined behind a Shiloh trait / module?  
- [ ] Will games need to name it in their `Cargo.toml`? (answer should be **no**)  
- [ ] Can we swap it without breaking scene/prefab formats?  

See also: [GRAPHICS.md](GRAPHICS.md) · [ROADMAP.md](ROADMAP.md)
