# Poly Haven CC0 assets (Powered by Poly Haven)

Meshes / textures / HDRIs from [Poly Haven](https://polyhaven.com) under
[CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/).  
Live API use requires a clear “Powered by Poly Haven” credit ([API terms](https://polyhaven.com/our-api)).

## Re-download / refresh

```bash
python3 shiloh-editor/assets/download_polyhaven.py
```

Skips models whose `.bin` exceeds ~60 MB (full `pine_tree_01` / `fir_tree_01` photogrammetry meshes are ~250–950 MB).

## What’s on disk

| Asset | Path | Notes |
|-------|------|--------|
| Shrub 03 | `props/shrub_03/` | Editor prop slot 0 |
| Fern 02 | `props/fern_02/` | Editor prop slot 1 |
| Rock 09 | `props/rock_09/` | Editor prop slot 2 |
| Rock 06 | `props/rock_06/` | Editor prop slot 3 |
| Pine sapling small | `props/pine_sapling_small/` | Valley pine LODs |
| Grass medium 01 | `props/grass_medium_01/` | Ground cover |
| Dead tree trunk | `props/dead_tree_trunk/` | Scatter |
| Rock moss set 01 | `props/rock_moss_set_01/` | Mossy rocks |
| Tree stump 01 | `props/tree_stump_01/` | Scatter |
| Forest leaves / ground | `textures/forest_*` | Terrain PBR maps |
| Aerial grass rock | `textures/aerial_grass_rock/` | Distant ground |
| Mountain midday HDRI | `hdris/fouriesburg_mountain_midday/` | Sky / lighting ref |
| Forest slope HDRI | `hdris/forest_slope/` | Forest lighting ref |
| Drakensberg PureSky | `hdris/drakensberg_solitary_mountain_puresky/` | Clear mountain sky ref |

Hero `pine_tree_01` / `fir_tree_01` texture packs may be present without `.bin` (too large for the repo). Run the script with a higher cap locally if you need full meshes.
