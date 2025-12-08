//! Per-voxel material storage and mesh integration for terrain texturing.
//!
//! This module provides [`MaterialField`] for storing material IDs, and utilities
//! to add material attributes to meshes. It's designed to integrate with
//! bevy-sculpter but has no hard dependency on it.
//!
//! # Feature Flag
//!
//! Requires the `material_field` feature:
//!
//! ```toml
//! bevy-painter = { version = "...", features = ["material_field"] }
//! ```
//!
//! # Example with bevy-sculpter
//!
//! ```ignore
//! use bevy_painter::prelude::*;
//! use bevy_sculpter::prelude::*;
//!
//! fn remesh(
//!     density: &DensityField,
//!     materials: &MaterialField,
//!     neighbors: &NeighborDensityFields,
//!     mesh_size: Vec3,
//! ) -> Option<Mesh> {
//!     let mut mesh = bevy_sculpter::mesher::generate_mesh_cpu(density, neighbors, mesh_size)?;
//!     add_material_attributes(
//!         &mut mesh,
//!         |x, y, z| density.get_signed(x, y, z).unwrap_or(1.0),
//!         &materials,
//!         mesh_size,
//!     );
//!     Some(mesh)
//! }
//! ```

use bevy::prelude::*;

use crate::mesh::{ATTRIBUTE_MATERIAL_IDS, ATTRIBUTE_MATERIAL_WEIGHTS};

/// Size of the field grid (matches bevy_sculpter::DENSITY_FIELD_SIZE).
pub const FIELD_SIZE: UVec3 = uvec3(32, 32, 32);
const FIELD_VOLUME: usize = (FIELD_SIZE.x * FIELD_SIZE.y * FIELD_SIZE.z) as usize;

/// Default material ID for uninitialized voxels.
pub const DEFAULT_MATERIAL: u8 = 0;

// ============================================================================
// MaterialField
// ============================================================================

/// Per-voxel material storage for a chunk (32KB).
#[derive(Component, Clone)]
pub struct MaterialField {
    data: Box<[u8; FIELD_VOLUME]>,
}

impl Default for MaterialField {
    fn default() -> Self {
        Self::new()
    }
}

impl MaterialField {
    pub fn new() -> Self {
        Self::filled(DEFAULT_MATERIAL)
    }

    pub fn filled(material_id: u8) -> Self {
        Self {
            data: Box::new([material_id; FIELD_VOLUME]),
        }
    }

    pub fn from_fn<F: FnMut(u32, u32, u32) -> u8>(mut f: F) -> Self {
        let mut data = Box::new([0u8; FIELD_VOLUME]);
        for z in 0..FIELD_SIZE.z {
            for y in 0..FIELD_SIZE.y {
                for x in 0..FIELD_SIZE.x {
                    data[Self::index(x, y, z)] = f(x, y, z);
                }
            }
        }
        Self { data }
    }

    #[inline]
    pub fn get(&self, x: u32, y: u32, z: u32) -> u8 {
        self.data[Self::index(x, y, z)]
    }

    #[inline]
    pub fn set(&mut self, x: u32, y: u32, z: u32, material_id: u8) {
        self.data[Self::index(x, y, z)] = material_id;
    }

    #[inline]
    pub fn get_clamped(&self, x: i32, y: i32, z: i32) -> u8 {
        let x = x.clamp(0, FIELD_SIZE.x as i32 - 1) as u32;
        let y = y.clamp(0, FIELD_SIZE.y as i32 - 1) as u32;
        let z = z.clamp(0, FIELD_SIZE.z as i32 - 1) as u32;
        self.get(x, y, z)
    }

    pub fn fill(&mut self, material_id: u8) {
        self.data.fill(material_id);
    }

    pub fn fill_by_height<F: FnMut(f32) -> u8>(&mut self, mut f: F) {
        for y in 0..FIELD_SIZE.y {
            let mat = f(y as f32);
            for z in 0..FIELD_SIZE.z {
                for x in 0..FIELD_SIZE.x {
                    self.set(x, y, z, mat);
                }
            }
        }
    }

    pub fn fill_by_world_height<F: FnMut(f32) -> u8>(
        &mut self,
        chunk_pos: IVec3,
        chunk_size: Vec3,
        mut f: F,
    ) {
        let voxel_h = chunk_size.y / FIELD_SIZE.y as f32;
        let base_y = chunk_pos.y as f32 * chunk_size.y;
        for y in 0..FIELD_SIZE.y {
            let world_y = base_y + y as f32 * voxel_h;
            let mat = f(world_y);
            for z in 0..FIELD_SIZE.z {
                for x in 0..FIELD_SIZE.x {
                    self.set(x, y, z, mat);
                }
            }
        }
    }

    #[inline]
    fn index(x: u32, y: u32, z: u32) -> usize {
        (z * FIELD_SIZE.y * FIELD_SIZE.x + y * FIELD_SIZE.x + x) as usize
    }
}

impl std::fmt::Debug for MaterialField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut counts = [0u32; 256];
        for &m in self.data.iter() {
            counts[m as usize] += 1;
        }
        let non_zero: Vec<_> = counts
            .iter()
            .enumerate()
            .filter(|&(_, &c)| c > 0)
            .collect();
        f.debug_struct("MaterialField")
            .field("distribution", &non_zero)
            .finish()
    }
}

// ============================================================================
// Mesh Integration
// ============================================================================

/// Adds material attributes to a mesh using a density sampler function.
pub fn add_material_attributes<F>(
    mesh: &mut Mesh,
    density_sampler: F,
    materials: &MaterialField,
    mesh_size: Vec3,
) where
    F: Fn(i32, i32, i32) -> f32,
{
    add_material_attributes_with(mesh, density_sampler, materials, mesh_size, &BlendConfig::default());
}

/// Like [`add_material_attributes`] but with custom blend settings.
pub fn add_material_attributes_with<F>(
    mesh: &mut Mesh,
    density_sampler: F,
    materials: &MaterialField,
    mesh_size: Vec3,
    config: &BlendConfig,
) where
    F: Fn(i32, i32, i32) -> f32,
{
    let positions = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(attr) => attr.as_float3().map(|p| p.to_vec()).unwrap_or_default(),
        None => return,
    };

    let scale = FIELD_SIZE.as_vec3() / mesh_size;
    let mut ids = Vec::with_capacity(positions.len());
    let mut weights = Vec::with_capacity(positions.len());

    for pos in &positions {
        let grid = Vec3::from(*pos) * scale;
        let cell = grid.floor().as_ivec3();
        let vm = compute_vertex_material(cell, &density_sampler, materials, config);
        ids.push(vm.0);
        weights.push(vm.1);
    }

    mesh.insert_attribute(ATTRIBUTE_MATERIAL_IDS, ids);
    mesh.insert_attribute(ATTRIBUTE_MATERIAL_WEIGHTS, weights);
}

/// Configuration for material blending.
#[derive(Clone, Debug)]
pub struct BlendConfig {
    pub density_weighted: bool,
    pub power: f32,
    pub min_weight: u8,
}

impl Default for BlendConfig {
    fn default() -> Self {
        Self {
            density_weighted: true,
            power: 1.0,
            min_weight: 5,
        }
    }
}

fn compute_vertex_material<F>(
    cell: IVec3,
    density_sampler: &F,
    materials: &MaterialField,
    config: &BlendConfig,
) -> (u32, u32)
where
    F: Fn(i32, i32, i32) -> f32,
{
    let mut contrib: [(u8, f32); 8] = [(0, 0.0); 8];
    let mut total = 0.0f32;

    for (i, off) in CUBE_CORNERS.iter().enumerate() {
        let p = cell + *off;
        let d = density_sampler(p.x, p.y, p.z);
        let m = materials.get_clamped(p.x, p.y, p.z);

        let w = if config.density_weighted {
            (-d).max(0.0).powf(config.power)
        } else {
            if d < 0.0 { 1.0 } else { 0.0 }
        };

        contrib[i] = (m, w);
        total += w;
    }

    if total < 0.0001 {
        return pack_single(contrib[0].0);
    }

    let mut agg: [(u8, f32); 4] = [(0, 0.0); 4];
    let mut count = 0usize;

    for (mat, w) in contrib {
        if w < 0.0001 {
            continue;
        }
        if let Some((_, aw)) = agg[..count].iter_mut().find(|(m, _)| *m == mat) {
            *aw += w;
        } else if count < 4 {
            agg[count] = (mat, w);
            count += 1;
        }
    }

    agg[..count].sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    let sum: f32 = agg.iter().map(|(_, w)| w).sum();
    if sum < 0.0001 {
        return pack_single(agg[0].0);
    }

    let mut ids = [0u8; 4];
    let mut wts = [0u8; 4];
    let mut running = 0u8;

    for (i, (mat, w)) in agg.iter().enumerate() {
        ids[i] = *mat;
        if i == 3 {
            wts[i] = 255 - running;
        } else {
            let normalized = ((w / sum) * 255.0).round() as u8;
            wts[i] = normalized.min(255 - running);
            running += wts[i];
        }
    }

    if config.min_weight > 0 {
        filter_low_weights(&mut wts, config.min_weight);
    }

    (u32::from_le_bytes(ids), u32::from_le_bytes(wts))
}

fn pack_single(mat: u8) -> (u32, u32) {
    (
        u32::from_le_bytes([mat, 0, 0, 0]),
        u32::from_le_bytes([255, 0, 0, 0]),
    )
}

fn filter_low_weights(wts: &mut [u8; 4], min: u8) {
    for w in wts.iter_mut() {
        if *w < min {
            *w = 0;
        }
    }
    let sum: u16 = wts.iter().map(|&w| w as u16).sum();
    if sum == 0 {
        wts[0] = 255;
        return;
    }
    if sum == 255 {
        return;
    }
    let scale = 255.0 / sum as f32;
    let mut running = 0u8;
    let mut last = 0;
    for (i, w) in wts.iter_mut().enumerate() {
        if *w > 0 {
            *w = ((*w as f32) * scale).round() as u8;
            running = running.saturating_add(*w);
            last = i;
        }
    }
    if running != 255 {
        wts[last] = wts[last].saturating_add(255 - running);
    }
}

const CUBE_CORNERS: [IVec3; 8] = [
    IVec3::new(0, 0, 0),
    IVec3::new(1, 0, 0),
    IVec3::new(0, 1, 0),
    IVec3::new(1, 1, 0),
    IVec3::new(0, 0, 1),
    IVec3::new(1, 0, 1),
    IVec3::new(0, 1, 1),
    IVec3::new(1, 1, 1),
];

// ============================================================================
// Brushes
// ============================================================================

/// Paints a sphere of material.
pub fn paint_sphere(field: &mut MaterialField, center: Vec3, radius: f32, material_id: u8) {
    let r2 = radius * radius;
    let min = (center - Vec3::splat(radius)).max(Vec3::ZERO).as_ivec3();
    let max = (center + Vec3::splat(radius))
        .min(FIELD_SIZE.as_vec3() - Vec3::ONE)
        .as_ivec3();

    for z in min.z..=max.z {
        for y in min.y..=max.y {
            for x in min.x..=max.x {
                let p = vec3(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5);
                if p.distance_squared(center) <= r2 {
                    field.set(x as u32, y as u32, z as u32, material_id);
                }
            }
        }
    }
}

/// Paints only voxels near the surface using a density sampler.
pub fn paint_surface<F>(
    materials: &mut MaterialField,
    density_sampler: F,
    center: Vec3,
    radius: f32,
    material_id: u8,
    threshold: f32,
) where
    F: Fn(u32, u32, u32) -> f32,
{
    let r2 = radius * radius;
    let min = (center - Vec3::splat(radius)).max(Vec3::ZERO).as_ivec3();
    let max = (center + Vec3::splat(radius))
        .min(FIELD_SIZE.as_vec3() - Vec3::ONE)
        .as_ivec3();

    for z in min.z..=max.z {
        for y in min.y..=max.y {
            for x in min.x..=max.x {
                let p = vec3(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5);
                if p.distance_squared(center) > r2 {
                    continue;
                }
                let d = density_sampler(x as u32, y as u32, z as u32);
                if d.abs() <= threshold {
                    materials.set(x as u32, y as u32, z as u32, material_id);
                }
            }
        }
    }
}

/// Fills materials by height layers: `&[(max_height, material_id)]`.
pub fn fill_height_layers(field: &mut MaterialField, layers: &[(f32, u8)]) {
    for y in 0..FIELD_SIZE.y {
        let h = y as f32;
        let mat = layers
            .iter()
            .find(|(max_h, _)| h < *max_h)
            .map(|(_, m)| *m)
            .unwrap_or(0);
        for z in 0..FIELD_SIZE.z {
            for x in 0..FIELD_SIZE.x {
                field.set(x, y, z, mat);
            }
        }
    }
}

/// Fills by surface steepness using a density sampler for gradients.
pub fn fill_by_steepness<F>(
    materials: &mut MaterialField,
    density_sampler: F,
    flat_mat: u8,
    steep_mat: u8,
    threshold: f32,
) where
    F: Fn(u32, u32, u32) -> f32,
{
    for z in 1..(FIELD_SIZE.z - 1) {
        for y in 1..(FIELD_SIZE.y - 1) {
            for x in 1..(FIELD_SIZE.x - 1) {
                let dx = density_sampler(x + 1, y, z) - density_sampler(x - 1, y, z);
                let dy = density_sampler(x, y + 1, z) - density_sampler(x, y - 1, z);
                let dz = density_sampler(x, y, z + 1) - density_sampler(x, y, z - 1);

                let grad = Vec3::new(dx, dy, dz);
                let len = grad.length();
                let steepness = if len > 0.001 {
                    1.0 - (grad.y / len).abs()
                } else {
                    0.0
                };

                let mat = if steepness > threshold { steep_mat } else { flat_mat };
                materials.set(x, y, z, mat);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_basics() {
        let mut f = MaterialField::new();
        assert_eq!(f.get(0, 0, 0), 0);
        f.set(5, 5, 5, 42);
        assert_eq!(f.get(5, 5, 5), 42);
    }

    #[test]
    fn test_pack_single() {
        let (ids, wts) = pack_single(7);
        assert_eq!(ids, u32::from_le_bytes([7, 0, 0, 0]));
        assert_eq!(wts, u32::from_le_bytes([255, 0, 0, 0]));
    }
}
