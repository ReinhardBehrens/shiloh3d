# Believable 3D slice — progress tracker

**Scope:** 3D only (iso camera for RTS/ARPG *feel*; no separate 2D engine yet).

**Must-have:** PBR textures · glTF · multi-light + shadows · skinned anim · iso camera · fog · water v1 · HUD · tonemap/grade

**Started:** 2026-07-29  

Legend: ⬜ Not started · 🔄 In progress · ✅ Done

---

## Checklist

| # | Item | Status | Location |
|---|---|---|---|
| 1 | PBR textures | ✅ | `SliceRenderer` + checker albedo + WGSL `pbr.wgsl` |
| 2 | glTF | ✅ | `shiloh-assets::load_gltf` (optional `sample.glb` in demo) |
| 3 | Multi-light + shadows | ✅ | Sun + 2 points; 2048 shadow map |
| 4 | Skinned anim | ✅ | `SkinPalette` + GPU skinned character sway |
| 5 | Iso camera | ✅ | `Camera::isometric` in demo |
| 6 | Fog | ✅ | Distance fog in PBR / water / skinned |
| 7 | Water v1 | ✅ | Transparent water plane pass |
| 8 | HUD | ✅ | Health/mana + hotbar NDC quads |
| 9 | Tonemap / grade | ✅ | HDR → ACES post |

## Done when

- [x] Iso demo view with PBR meshes
- [x] 1 directional + ≥2 point lights + sun shadow
- [x] Skinned mesh animating
- [x] Water + fog visible
- [x] HUD + tonemapped frame
- [x] `cargo run -p shiloh-demo --release` works; headless still OK

## Session log

| Date | Change |
|---|---|
| 2026-07-29 | Tracker created; 3D-only policy confirmed; slice implementation |
| 2026-07-29 | Phase 1 ECS/hierarchy/textures/app host + Phase 2 SliceRenderer wired into demo |
