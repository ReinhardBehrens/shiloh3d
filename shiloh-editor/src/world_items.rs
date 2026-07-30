//! Built-in world item catalog and project asset browser state.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldItemCategory {
    Environment,
    Terrain,
    Foliage,
    Props,
    Lighting,
}

impl WorldItemCategory {
    pub fn label(self) -> &'static str {
        match self {
            Self::Environment => "Environment",
            Self::Terrain => "Terrain",
            Self::Foliage => "Foliage",
            Self::Props => "Props",
            Self::Lighting => "Lighting",
        }
    }
}

#[derive(Debug, Clone)]
pub struct WorldItem {
    pub id: &'static str,
    pub name: &'static str,
    pub category: WorldItemCategory,
    pub description: &'static str,
    /// Suggested relative spawn name when dropped into the scene.
    pub spawn_name: &'static str,
}

/// Seed catalog matching the premium editor mockup (pines, birches, rocks, water…).
pub fn builtin_world_items() -> Vec<WorldItem> {
    vec![
        WorldItem {
            id: "pine_tall",
            name: "Pine Tall",
            category: WorldItemCategory::Foliage,
            description: "Tall conifer for forest ridges",
            spawn_name: "Pine_Tall",
        },
        WorldItem {
            id: "pine_cluster",
            name: "Pine Cluster",
            category: WorldItemCategory::Foliage,
            description: "Grouped pines for density",
            spawn_name: "Pine_Cluster",
        },
        WorldItem {
            id: "birch",
            name: "Birch",
            category: WorldItemCategory::Foliage,
            description: "Light deciduous tree",
            spawn_name: "Birch",
        },
        WorldItem {
            id: "dead_tree",
            name: "Dead Tree",
            category: WorldItemCategory::Foliage,
            description: "Weathered trunk / silhouette",
            spawn_name: "Dead_Tree",
        },
        WorldItem {
            id: "grass_patch",
            name: "Grass Patch",
            category: WorldItemCategory::Foliage,
            description: "Ground cover scatter (Fern_02 prop when loaded)",
            spawn_name: "Grass_Patch",
        },
        WorldItem {
            id: "shrub_03",
            name: "Shrub (Poly Haven)",
            category: WorldItemCategory::Foliage,
            description: "CC0 shrub photogrammetry · shrub_03",
            spawn_name: "Shrub_03",
        },
        WorldItem {
            id: "fern_02",
            name: "Fern (Poly Haven)",
            category: WorldItemCategory::Foliage,
            description: "CC0 small fern · fern_02",
            spawn_name: "Fern_02",
        },
        WorldItem {
            id: "rock_large",
            name: "Rock Large",
            category: WorldItemCategory::Props,
            description: "Hero boulder (Rock_09 prop when loaded)",
            spawn_name: "Rock_09",
        },
        WorldItem {
            id: "rock_scatter",
            name: "Rock Scatter",
            category: WorldItemCategory::Props,
            description: "Small stone (Rock_06 prop when loaded)",
            spawn_name: "Rock_06",
        },
        WorldItem {
            id: "rock_photogrammetry",
            name: "Rock Photogrammetry",
            category: WorldItemCategory::Props,
            description: "CC0 hero rock · rock_09",
            spawn_name: "Rock_09",
        },
        WorldItem {
            id: "cliff",
            name: "Cliff Face",
            category: WorldItemCategory::Terrain,
            description: "Vertical rock wall piece",
            spawn_name: "Cliff_Face",
        },
        WorldItem {
            id: "heightmap",
            name: "Heightmap Terrain",
            category: WorldItemCategory::Terrain,
            description: "Terrain chunk placeholder (blending stub — not yet implemented)",
            spawn_name: "Terrain_Heightmap",
        },
        WorldItem {
            id: "water_body",
            name: "Water Body",
            category: WorldItemCategory::Environment,
            description: "Lake / river plane (slice water v1+)",
            spawn_name: "WaterBody",
        },
        WorldItem {
            id: "sky_atmosphere",
            name: "Sky & Atmosphere",
            category: WorldItemCategory::Environment,
            description: "Sky dome + fog volume",
            spawn_name: "SkyAtmosphere",
        },
        WorldItem {
            id: "fog_volume",
            name: "Fog Volume",
            category: WorldItemCategory::Environment,
            description: "Local atmospheric fog",
            spawn_name: "FogVolume",
        },
        WorldItem {
            id: "dir_light",
            name: "Directional Light",
            category: WorldItemCategory::Lighting,
            description: "Sun / moon key light",
            spawn_name: "DirectionalLight",
        },
        WorldItem {
            id: "point_light",
            name: "Point Light",
            category: WorldItemCategory::Lighting,
            description: "Omni local light",
            spawn_name: "PointLight",
        },
        WorldItem {
            id: "spot_light",
            name: "Spot Light",
            category: WorldItemCategory::Lighting,
            description: "Cone spotlight (slice spot path)",
            spawn_name: "SpotLight",
        },
    ]
}

#[derive(Debug, Clone)]
pub struct ProjectAsset {
    pub name: String,
    pub path: PathBuf,
    pub kind: AssetKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetKind {
    Mesh,
    Texture,
    Material,
    Scene,
    Other,
}

impl AssetKind {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_ascii_lowercase().as_str() {
            "gltf" | "glb" | "obj" | "fbx" => Self::Mesh,
            "png" | "jpg" | "jpeg" | "webp" | "ktx2" | "hdr" | "exr" => Self::Texture,
            "mat" => Self::Material,
            _ => Self::Other,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Mesh => "Mesh",
            Self::Texture => "Texture",
            Self::Material => "Material",
            Self::Scene => "Scene",
            Self::Other => "File",
        }
    }
}

/// Scan `project/assets` (and common subfolders) for browser entries.
pub fn scan_project_assets(root: &Path) -> Vec<ProjectAsset> {
    let assets = root.join("assets");
    let mut out = Vec::new();
    if !assets.exists() {
        return out;
    }
    scan_dir(&assets, &assets, &mut out);
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn scan_dir(root: &Path, dir: &Path, out: &mut Vec<ProjectAsset>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_dir(root, &path, out);
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string();
        let _ = root;
        out.push(ProjectAsset {
            name: name.to_string(),
            path,
            kind: AssetKind::from_extension(&ext),
        });
    }
}

/// Ensure premium project folders exist on disk.
pub fn ensure_project_layout(root: &Path) -> std::io::Result<()> {
    for sub in [
        "assets/Environment",
        "assets/Foliage",
        "assets/Materials",
        "assets/Meshes",
        "assets/Textures",
        "assets/Imported",
        "scenes",
        "scripts/graphs",
    ] {
        std::fs::create_dir_all(root.join(sub))?;
    }
    Ok(())
}
