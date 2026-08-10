//! Heightmap terrain chunk with 4-layer splat weights (Phase 5 Landscape).

use serde::{Deserialize, Serialize};

/// Grass / dirt / rock / sand weight layers (must sum ≈ 1 after paint).
pub const TERRAIN_LAYER_COUNT: usize = 4;

/// Heightmap + splat weights for one streamed terrain tile.
///
/// Grid samples cover world XZ in `[0, world_size] × [0, world_size]`.
/// Sampled world height is `heights[i] * height_scale`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerrainChunk {
    pub width: u32,
    pub height: u32,
    pub world_size: f32,
    pub height_scale: f32,
    /// Row-major height samples (`height * width`).
    pub heights: Vec<f32>,
    /// Row-major splat weights per sample (grass, dirt, rock, sand).
    pub weights: Vec<[f32; TERRAIN_LAYER_COUNT]>,
}

impl Default for TerrainChunk {
    fn default() -> Self {
        Self::flat(128, 64.0)
    }
}

impl TerrainChunk {
    /// Flat grass tile: heights 0, weights `[1, 0, 0, 0]`.
    pub fn flat(resolution: u32, world_size: f32) -> Self {
        let res = resolution.max(2);
        let n = (res as usize) * (res as usize);
        Self {
            width: res,
            height: res,
            world_size: world_size.max(1e-3),
            height_scale: 8.0,
            heights: vec![0.0; n],
            weights: vec![[1.0, 0.0, 0.0, 0.0]; n],
        }
    }

    #[inline]
    fn index(&self, ix: u32, iz: u32) -> usize {
        (iz as usize) * (self.width as usize) + (ix as usize)
    }

    #[inline]
    fn world_to_grid(&self, x: f32, z: f32) -> (f32, f32) {
        let w = self.world_size.max(1e-3);
        let gx = (x / w) * (self.width.saturating_sub(1) as f32);
        let gz = (z / w) * (self.height.saturating_sub(1) as f32);
        (gx, gz)
    }

    /// Bilinear height at world XZ (includes `height_scale`).
    pub fn height_at_world(&self, x: f32, z: f32) -> f32 {
        if self.heights.is_empty() || self.width < 2 || self.height < 2 {
            return 0.0;
        }
        let (gx, gz) = self.world_to_grid(x, z);
        let max_x = (self.width - 1) as f32;
        let max_z = (self.height - 1) as f32;
        let gx = gx.clamp(0.0, max_x);
        let gz = gz.clamp(0.0, max_z);
        let x0 = gx.floor() as u32;
        let z0 = gz.floor() as u32;
        let x1 = (x0 + 1).min(self.width - 1);
        let z1 = (z0 + 1).min(self.height - 1);
        let tx = gx - x0 as f32;
        let tz = gz - z0 as f32;
        let h00 = self.heights[self.index(x0, z0)];
        let h10 = self.heights[self.index(x1, z0)];
        let h01 = self.heights[self.index(x0, z1)];
        let h11 = self.heights[self.index(x1, z1)];
        let h0 = h00 + (h10 - h00) * tx;
        let h1 = h01 + (h11 - h01) * tx;
        (h0 + (h1 - h0) * tz) * self.height_scale
    }

    /// Raise (`strength > 0`) or lower (`strength < 0`) with smooth brush falloff.
    pub fn sculpt(&mut self, x: f32, z: f32, radius: f32, strength: f32) {
        let radius = radius.max(1e-3);
        let r2 = radius * radius;
        let (cx, cz) = self.world_to_grid(x, z);
        let cell = self.world_size / self.width.saturating_sub(1).max(1) as f32;
        let brush_cells = (radius / cell.max(1e-3)).ceil() as i32 + 1;
        let ix0 = (cx as i32 - brush_cells).max(0) as u32;
        let iz0 = (cz as i32 - brush_cells).max(0) as u32;
        let ix1 = ((cx as i32 + brush_cells) as u32).min(self.width.saturating_sub(1));
        let iz1 = ((cz as i32 + brush_cells) as u32).min(self.height.saturating_sub(1));

        for iz in iz0..=iz1 {
            for ix in ix0..=ix1 {
                let (wx, wz) = self.grid_to_world(ix, iz);
                let dx = wx - x;
                let dz = wz - z;
                let d2 = dx * dx + dz * dz;
                if d2 > r2 {
                    continue;
                }
                let t = 1.0 - (d2 / r2).sqrt();
                // Smooth falloff (cubic smoothstep-ish).
                let falloff = t * t * (3.0 - 2.0 * t);
                let i = self.index(ix, iz);
                self.heights[i] += strength * falloff;
            }
        }
    }

    /// Paint splat layer `layer` (0..3) with brush falloff; renormalizes weights.
    pub fn paint_layer(&mut self, x: f32, z: f32, radius: f32, strength: f32, layer: usize) {
        if layer >= TERRAIN_LAYER_COUNT {
            return;
        }
        let radius = radius.max(1e-3);
        let r2 = radius * radius;
        let strength = strength.clamp(0.0, 1.0);
        let (cx, cz) = self.world_to_grid(x, z);
        let cell = self.world_size / self.width.saturating_sub(1).max(1) as f32;
        let brush_cells = (radius / cell.max(1e-3)).ceil() as i32 + 1;
        let ix0 = (cx as i32 - brush_cells).max(0) as u32;
        let iz0 = (cz as i32 - brush_cells).max(0) as u32;
        let ix1 = ((cx as i32 + brush_cells) as u32).min(self.width.saturating_sub(1));
        let iz1 = ((cz as i32 + brush_cells) as u32).min(self.height.saturating_sub(1));

        for iz in iz0..=iz1 {
            for ix in ix0..=ix1 {
                let (wx, wz) = self.grid_to_world(ix, iz);
                let dx = wx - x;
                let dz = wz - z;
                let d2 = dx * dx + dz * dz;
                if d2 > r2 {
                    continue;
                }
                let t = 1.0 - (d2 / r2).sqrt();
                let falloff = t * t * (3.0 - 2.0 * t);
                let blend = (strength * falloff).clamp(0.0, 1.0);
                let i = self.index(ix, iz);
                let mut w = self.weights[i];
                for l in 0..TERRAIN_LAYER_COUNT {
                    if l == layer {
                        w[l] = w[l] + (1.0 - w[l]) * blend;
                    } else {
                        w[l] *= 1.0 - blend;
                    }
                }
                normalize_weights(&mut w);
                self.weights[i] = w;
            }
        }
    }

    #[inline]
    pub fn grid_to_world(&self, ix: u32, iz: u32) -> (f32, f32) {
        let max_x = self.width.saturating_sub(1).max(1) as f32;
        let max_z = self.height.saturating_sub(1).max(1) as f32;
        (
            (ix as f32 / max_x) * self.world_size,
            (iz as f32 / max_z) * self.world_size,
        )
    }
}

fn normalize_weights(w: &mut [f32; TERRAIN_LAYER_COUNT]) {
    let sum = w.iter().sum::<f32>();
    if sum > 1e-6 {
        for v in w.iter_mut() {
            *v /= sum;
        }
    } else {
        *w = [1.0, 0.0, 0.0, 0.0];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_defaults() {
        let t = TerrainChunk::flat(16, 32.0);
        assert_eq!(t.width, 16);
        assert_eq!(t.height, 16);
        assert_eq!(t.world_size, 32.0);
        assert_eq!(t.height_scale, 8.0);
        assert_eq!(t.heights.len(), 256);
        assert_eq!(t.weights.len(), 256);
        assert!(t.heights.iter().all(|&h| h == 0.0));
        assert!(t.weights.iter().all(|w| *w == [1.0, 0.0, 0.0, 0.0]));
        assert!((t.height_at_world(16.0, 16.0)).abs() < 1e-5);
    }

    #[test]
    fn sculpt_raises_center() {
        let mut t = TerrainChunk::flat(33, 32.0);
        t.sculpt(16.0, 16.0, 8.0, 1.0);
        let center = t.height_at_world(16.0, 16.0);
        let edge = t.height_at_world(0.0, 0.0);
        assert!(center > edge);
        assert!(center > 0.0);
        // Strength 1 at brush center → heights ≈ 1 → world height ≈ height_scale.
        assert!((center - t.height_scale).abs() < 0.5);
    }

    #[test]
    fn paint_layer_blends_dirt() {
        let mut t = TerrainChunk::flat(17, 16.0);
        t.paint_layer(8.0, 8.0, 4.0, 1.0, 1);
        let mid = 8 * t.width as usize + 8;
        let dirt = t.weights[mid][1];
        let grass = t.weights[mid][0];
        assert!(dirt > 0.9);
        assert!(grass < 0.1);
        let sum: f32 = t.weights[mid].iter().sum();
        assert!((sum - 1.0).abs() < 1e-4);
        // Corner stays grass.
        assert_eq!(t.weights[0], [1.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn serde_roundtrip() {
        let t = TerrainChunk::flat(4, 8.0);
        let json = serde_json::to_string(&t).expect("serialize");
        let back: TerrainChunk = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.width, t.width);
        assert_eq!(back.heights, t.heights);
        assert_eq!(back.weights, t.weights);
    }
}
