# Believable 3D slice — progress tracker

**Scope:** 3D only (iso camera for RTS/ARPG *feel*; no separate 2D engine yet).

**Quality bar:** [QUALITY_BAR.md](QUALITY_BAR.md) — Blackmarsh swamp ARPG (user-confirmed).

**Must-have:** PBR textures · glTF · multi-light + shadows · skinned anim · iso camera · fog · water v1 · HUD · tonemap/grade

**Started:** 2026-07-29  

Legend: ⬜ Not started · 🔄 In progress · ✅ Done

---

## Checklist

| # | Item | Status | Location |
|---|---|---|---|
| 1 | PBR textures | ✅ | `SliceRenderer` + albedo + WGSL `pbr.wgsl` |
| 2 | glTF | ✅ | `load_gltf` → `set_extra_mesh`; `assets/sample.gltf` |
| 3 | Multi-light + shadows | ✅ | Sun + 2 points; 2048 shadow map |
| 4 | Skinned anim | ✅ | `SkinPalette` + GPU skinned character sway |
| 5 | Iso camera | ✅ | `Camera::isometric` in demo |
| 6 | Fog | ✅ | Distance fog in PBR / water / skinned |
| 7 | Water v1 | ✅ | Transparent water plane pass |
| 8 | HUD | ✅ | Health/mana + hotbar NDC quads |
| 9 | Tonemap / grade | ✅ | HDR → ACES post |
| 10 | Physics → entity | ✅ | Stub body position syncs `Transform` + visible ball |
| 11 | Audio one-shot | ✅ | `AudioClip::sine_beep` + `play_oneshot` mixed |
| 12 | Editor H/I/Play | ✅ | `cargo run -p shiloh-editor` |

## Done when

- [x] Iso demo view with PBR meshes
- [x] 1 directional + ≥2 point lights + sun shadow
- [x] Skinned mesh animating
- [x] Water + fog visible
- [x] HUD + tonemapped frame
- [x] glTF mesh on GPU under slice lighting
- [x] Physics moves a transform; audio one-shot mixes non-silent
- [x] Editor hierarchy / inspector / play mode
- [x] `cargo run -p shiloh-demo` works; headless still OK

## Session log

| Date | Change |
|---|---|
| 2026-07-29 | Tracker created; 3D-only policy confirmed; slice implementation |
| 2026-07-29 | Phase 1 ECS/hierarchy/textures/app host + Phase 2 SliceRenderer wired into demo |
| 2026-07-29 | Phase 2 exit closed: glTF→GPU, physics+audio, editor; Blackmarsh bar confirmed |
| 2026-07-29 | User locked **real water + atmosphere/effects** as presentation bar (water v2+ next) |
