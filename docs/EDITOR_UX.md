# Shiloh Studio — Editor UX contract (Phase 5)

Godot-simple docks + Unreal Modes for world building.  
Blackmarsh ARPG shot = complexity bar; FirstGoal mockup = presentation bar.  
Christian-owned engine meant to be **bundled into other games**.

Every borrowed control in code must carry:

```rust
// Borrowed from Godot 4: …
// Borrowed from Unreal Engine: …
```

---

## Love / hate (research → rules)

| Source | Love (copy) | Hate (refuse) |
|---|---|---|
| **Godot** | Scene tree, FileSystem, Inspector docks; QWER; layouts; drag FS→Inspector; bottom panels | Hiding Scene tree in Script mode; dual-bound shortcuts; panel chaos |
| **Unreal** | Modes Shift+1..3; Landscape/Foliage; Outliner search; Content drawer; Game view G; F focus; snap | DIY material graph before paint; too many modes on day one |
| **Blender** | World shaping quality | Not a game runtime — keep as glTF **peer**, not in-editor Blender |

### Hard rules

1. **One editing context** — Script / Game never hide Outliner + Inspector.  
2. **Paint before graph** — four default terrain layers work with no material graph.  
3. **Shortcuts are exclusive** — one action per binding.  
4. **Status bar** always shows mode · tool · backend · FPS · context hint.  
5. **Blender is a peer** — document cook/LOD/collision; do not rebuild Blender inside Studio.

---

## Borrow map

### Godot 4 → Shiloh

| Pattern | Shiloh |
|---|---|
| Scene · FileSystem · Viewport · Inspector · bottom | Studio dock shell |
| Editor Layouts | `Window → Layouts` → `.shiloh/layouts/*.json` |
| 3D · Script · Game | Top workspace strip |
| Q W E R | Select / Move / Rotate / Scale |
| Ctrl+D | Duplicate selection |
| Distraction-free | Hide side docks |
| FS → Inspector drag | Path onto matching properties |

### Unreal Engine → Shiloh

| Pattern | Shiloh |
|---|---|
| Shift+1 / 2 / 3 | Select / Landscape / Foliage Modes |
| Landscape sculpt + paint | Height + 4 weight layers (defaults) |
| Foliage paint | Density, scale variance, align-to-normal |
| Outliner search / filters | Filter box + type chips |
| Details search | Inspector filter |
| Ctrl+Space Content drawer | Overlay asset browser |
| G game view | Hide gizmos |
| F focus | Frame selection |
| Grid snap + Ctrl free | Viewport snap |
| Alt+drag duplicate-move | Duplicate along drag |

### Not borrowed

- Godot Script workspace that removes the Scene tree  
- Unreal Modeling / Fracture / Animation modes (Phase 5)  
- Marketplace / Quixel-style storefront  

---

## Modes & shortcuts

| Mode | Shortcut | Tools |
|---|---|---|
| Select | Shift+1 / Q | Pick, multi-select |
| Move | W | Axis gizmo, grid snap |
| Rotate | E | Axis rotate |
| Scale | R | Axis scale |
| Landscape | Shift+2 | Sculpt / layer paint |
| Foliage | Shift+3 | Paint / erase instances |
| Place | Asset brush | Click ground |
| Script | Top Script | Rhai + visual (tree stays) |

Camera: LMB edit · MMB orbit · RMB pan · scroll zoom (Godot `_forward_3d_gui_input` contract).

---

## Scripting tiers (authoring)

| Tier | Role |
|---|---|
| Rust `ScriptModule` | Plugins / ship code |
| **Rhai** | Designer scripts (GDScript niche) |
| Visual graph | Blueprint-lite Event→Action |
| JS/Boa | Deferred |

Host API only: transform, spawn, input, timers, signals — never raw archetype ECS.

See [ROADMAP.md](ROADMAP.md) Phase 5 · [PREMIUM.md](PREMIUM.md) · [QUALITY_BAR.md](QUALITY_BAR.md).
