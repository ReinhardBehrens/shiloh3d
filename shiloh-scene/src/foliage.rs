//! Instanced foliage paint layer (Phase 5 Foliage Mode).

use serde::{Deserialize, Serialize};

/// Stable type id for a foliage mesh / prefab (`"pine"`, `"birch"`, …).
pub type FoliageTypeId = String;

/// One placed foliage instance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FoliageInstance {
    pub typ: FoliageTypeId,
    pub translation: [f32; 3],
    pub yaw: f32,
    pub scale: f32,
}

/// Brushable set of instances with density / align settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FoliageLayer {
    pub instances: Vec<FoliageInstance>,
    /// Instances per square world-unit when painting.
    pub density: f32,
    pub align_to_normal: bool,
}

impl Default for FoliageLayer {
    fn default() -> Self {
        Self {
            instances: Vec::new(),
            density: 0.25,
            align_to_normal: true,
        }
    }
}

impl FoliageLayer {
    /// Scatter instances near `xz` using `density`, random yaw, and scale variance.
    ///
    /// `scale_variance` is a fraction of `scale` (e.g. `0.2` → ±20%).
    /// `seed` drives a local LCG so tests stay deterministic without extra crates.
    pub fn paint_add(
        &mut self,
        typ: impl Into<FoliageTypeId>,
        xz: [f32; 2],
        y: f32,
        radius: f32,
        scale: f32,
        scale_variance: f32,
        seed: u64,
    ) {
        let typ = typ.into();
        let radius = radius.max(1e-3);
        let area = std::f32::consts::PI * radius * radius;
        let count = ((self.density.max(0.0) * area).round() as usize).max(1);
        let mut rng = seed | 1;
        for _ in 0..count {
            let u = next_f32(&mut rng);
            let v = next_f32(&mut rng);
            // Uniform disk sample.
            let r = radius * u.sqrt();
            let theta = v * std::f32::consts::TAU;
            let x = xz[0] + r * theta.cos();
            let z = xz[1] + r * theta.sin();
            let yaw = next_f32(&mut rng) * std::f32::consts::TAU;
            let var = scale_variance.abs().min(1.0);
            let s = scale * (1.0 + (next_f32(&mut rng) * 2.0 - 1.0) * var);
            self.instances.push(FoliageInstance {
                typ: typ.clone(),
                translation: [x, y, z],
                yaw,
                scale: s.max(1e-3),
            });
        }
    }

    /// Remove instances whose XZ lies within `radius` of `xz`.
    pub fn erase(&mut self, xz: [f32; 2], radius: f32) {
        let r2 = radius.max(0.0) * radius.max(0.0);
        self.instances.retain(|inst| {
            let dx = inst.translation[0] - xz[0];
            let dz = inst.translation[2] - xz[1];
            dx * dx + dz * dz > r2
        });
    }
}

/// Tiny LCG → [0, 1).
fn next_f32(state: &mut u64) -> f32 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1);
    ((*state >> 33) as u32 as f32) * (1.0 / (u32::MAX as f32 + 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paint_add_places_instances() {
        let mut layer = FoliageLayer {
            density: 1.0,
            ..Default::default()
        };
        layer.paint_add("pine", [0.0, 0.0], 0.0, 2.0, 1.0, 0.2, 42);
        assert!(!layer.instances.is_empty());
        assert!(layer.instances.iter().all(|i| i.typ == "pine"));
        assert!(layer.instances.iter().all(|i| {
            let dx = i.translation[0];
            let dz = i.translation[2];
            dx * dx + dz * dz <= 2.0 * 2.0 + 1e-3
        }));
        assert!(layer.instances.iter().all(|i| i.scale > 0.5 && i.scale < 1.5));
    }

    #[test]
    fn erase_removes_nearby() {
        let mut layer = FoliageLayer::default();
        layer.instances.push(FoliageInstance {
            typ: "birch".into(),
            translation: [1.0, 0.0, 0.0],
            yaw: 0.0,
            scale: 1.0,
        });
        layer.instances.push(FoliageInstance {
            typ: "birch".into(),
            translation: [10.0, 0.0, 0.0],
            yaw: 0.0,
            scale: 1.0,
        });
        layer.erase([0.0, 0.0], 2.0);
        assert_eq!(layer.instances.len(), 1);
        assert_eq!(layer.instances[0].translation[0], 10.0);
    }

    #[test]
    fn serde_roundtrip() {
        let mut layer = FoliageLayer::default();
        layer.paint_add("rock", [3.0, 4.0], 1.5, 1.0, 0.8, 0.1, 7);
        let json = serde_json::to_string(&layer).expect("serialize");
        let back: FoliageLayer = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.density, layer.density);
        assert_eq!(back.instances.len(), layer.instances.len());
        assert_eq!(back.instances[0].typ, "rock");
    }
}
