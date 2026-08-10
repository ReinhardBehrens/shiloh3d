# Blender → Shiloh asset pipeline (Phase 5)

Blender is a **peer** for hero modeling — not rebuilt inside Studio.
Export glTF 2.0, drop under `Assets/`, then cook stubs for collision + LOD.

## Export from Blender

1. Apply scale / rotation (`Ctrl+A`).
2. Export **glTF 2.0** (`.glb` preferred, or `.gltf` + `.bin` + textures).
3. Enable: UVs, Normals, Materials; PBR base color / metallic-roughness / normal.
4. Place under `shiloh_project/Assets/Meshes/` (or `Foliage/`, `Props/`).

## Import / cook stubs

Studio URL import or file copy → `Assets/Imported/`.

Cook metadata (written beside the mesh as `*.shiloh.json`):

```json
{
  "source": "Assets/Meshes/pine_hero.glb",
  "collision": { "kind": "box", "half_extents": [0.8, 2.4, 0.8] },
  "lod": [
    { "distance": 0.0, "mesh": "pine_hero.glb" },
    { "distance": 40.0, "mesh": "pine_hero_lod1.glb" }
  ]
}
```

`shiloh-editor` / `shiloh-cli` may generate a **box hull stub** from mesh AABB when collision is missing.

## LOD

- LOD0: full mesh  
- LOD1: optional lower glTF or billboard stub in foliage mode  
- Distance fields are authored in the JSON stub above  

## Thumbnails

Asset Browser prefers `*_thumb.png` next to the glTF; otherwise a flat category icon.

## Do not

- Expect Blender Geometry Nodes graphs to run in-engine  
- Skip materials on export (vertex-color-only is a last resort)  

See [EDITOR_UX.md](EDITOR_UX.md) · Phase 5 in [ROADMAP.md](ROADMAP.md).
