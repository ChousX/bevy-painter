//! Material blending logic based on density values.

use bevy::prelude::*;
use bevy_sculpter::prelude::DensityField;

use super::field::{MaterialField, FIELD_SIZE};
use super::neighbor::{NeighborFace, NeighborMaterialFields};
use crate::mesh::VertexMaterialData;

/// Settings for material blending at vertices.
#[derive(Resource, Clone, Debug)]
pub struct MaterialBlendSettings {
    /// How much negative density contributes to material weight.
    /// Higher values = sharper transitions between materials.
    /// Default: 2.0
    pub density_influence: f32,

    /// Minimum weight threshold to include a material in blending.
    /// Materials below this weight are excluded.
    /// Default: 0.01
    pub weight_threshold: f32,
}

impl Default for MaterialBlendSettings {
    fn default() -> Self {
        Self {
            density_influence: 2.0,
            weight_threshold: 0.01,
        }
    }
}

/// Offsets to the 8 corners of a voxel cube.
const CORNER_OFFSETS: [[i32; 3]; 8] = [
    [0, 0, 0],
    [1, 0, 0],
    [0, 1, 0],
    [1, 1, 0],
    [0, 0, 1],
    [1, 0, 1],
    [0, 1, 1],
    [1, 1, 1],
];

/// Computes material blend data for a vertex at the given world position.
///
/// Samples the 8 surrounding voxels and blends their materials based on
/// how "inside" each voxel is (negative density = inside).
pub fn compute_vertex_materials(
    world_pos: Vec3,
    mesh_size: Vec3,
    density_field: &DensityField,
    material_field: &MaterialField,
    neighbor_densities: Option<&bevy_sculpter::neighbor::NeighborDensityFields>,
    neighbor_materials: Option<&NeighborMaterialFields>,
    settings: &MaterialBlendSettings,
) -> VertexMaterialData {
    let scale = FIELD_SIZE.as_vec3() / mesh_size;
    let grid_pos = world_pos * scale;

    // Find the base voxel (floor of grid position)
    let base = grid_pos.floor().as_ivec3();

    // Collect materials and their weights from 8 surrounding voxels
    let mut contributions: Vec<(u8, f32)> = Vec::with_capacity(8);

    for offset in &CORNER_OFFSETS {
        let voxel = base + IVec3::from_slice(offset);

        let density = sample_density(voxel, density_field, neighbor_densities);
        let material = sample_material(voxel, material_field, neighbor_materials);

        // Convert density to weight: more negative = more "inside" = higher weight
        // Only interior voxels (negative density) contribute
        if density < 0.0 {
            let weight = (-density * settings.density_influence).clamp(0.0, 1.0);
            if weight > settings.weight_threshold {
                contributions.push((material, weight));
            }
        }
    }

    // If no interior voxels, use nearest voxel's material
    if contributions.is_empty() {
        let nearest = grid_pos.round().as_ivec3();
        let material = sample_material(nearest, material_field, neighbor_materials);
        return VertexMaterialData::single(material);
    }

    // Merge duplicate materials and normalize weights
    merge_and_normalize_materials(&mut contributions);

    // Convert to VertexMaterialData (up to 4 materials)
    contributions_to_vertex_data(&contributions)
}

/// Samples density at a voxel coordinate, handling neighbor lookups.
fn sample_density(
    voxel: IVec3,
    field: &DensityField,
    neighbors: Option<&bevy_sculpter::neighbor::NeighborDensityFields>,
) -> f32 {
    let size = FIELD_SIZE.as_ivec3();

    // In bounds - direct sample
    if voxel.x >= 0 && voxel.y >= 0 && voxel.z >= 0 
        && voxel.x < size.x && voxel.y < size.y && voxel.z < size.z 
    {
        return field.get(voxel.x as u32, voxel.y as u32, voxel.z as u32);
    }

    // Try neighbor lookup
    if let Some(neighbors) = neighbors {
        if let Some(density) = sample_density_neighbor(voxel, neighbors) {
            return density;
        }
    }

    // Out of bounds, no neighbor - return exterior
    1.0
}

/// Samples density from neighbor fields.
fn sample_density_neighbor(
    voxel: IVec3,
    neighbors: &bevy_sculpter::neighbor::NeighborDensityFields,
) -> Option<f32> {
    let size = FIELD_SIZE.as_ivec3();

    // -X neighbor
    if voxel.x < 0 && voxel.y >= 0 && voxel.z >= 0 && voxel.y < size.y && voxel.z < size.z {
        if let Some(ref slice) = neighbors.neighbors[NeighborFace::NegX as usize] {
            let depth = (-1 - voxel.x) as u32;
            return Some(slice.get(voxel.y as u32, voxel.z as u32, depth));
        }
    }

    // +X neighbor
    if voxel.x >= size.x && voxel.y >= 0 && voxel.z >= 0 && voxel.y < size.y && voxel.z < size.z {
        if let Some(ref slice) = neighbors.neighbors[NeighborFace::PosX as usize] {
            let depth = (voxel.x - size.x) as u32;
            return Some(slice.get(voxel.y as u32, voxel.z as u32, depth));
        }
    }

    // -Y neighbor
    if voxel.y < 0 && voxel.x >= 0 && voxel.z >= 0 && voxel.x < size.x && voxel.z < size.z {
        if let Some(ref slice) = neighbors.neighbors[NeighborFace::NegY as usize] {
            let depth = (-1 - voxel.y) as u32;
            return Some(slice.get(voxel.x as u32, voxel.z as u32, depth));
        }
    }

    // +Y neighbor
    if voxel.y >= size.y && voxel.x >= 0 && voxel.z >= 0 && voxel.x < size.x && voxel.z < size.z {
        if let Some(ref slice) = neighbors.neighbors[NeighborFace::PosY as usize] {
            let depth = (voxel.y - size.y) as u32;
            return Some(slice.get(voxel.x as u32, voxel.z as u32, depth));
        }
    }

    // -Z neighbor
    if voxel.z < 0 && voxel.x >= 0 && voxel.y >= 0 && voxel.x < size.x && voxel.y < size.y {
        if let Some(ref slice) = neighbors.neighbors[NeighborFace::NegZ as usize] {
            let depth = (-1 - voxel.z) as u32;
            return Some(slice.get(voxel.x as u32, voxel.y as u32, depth));
        }
    }

    // +Z neighbor
    if voxel.z >= size.z && voxel.x >= 0 && voxel.y >= 0 && voxel.x < size.x && voxel.y < size.y {
        if let Some(ref slice) = neighbors.neighbors[NeighborFace::PosZ as usize] {
            let depth = (voxel.z - size.z) as u32;
            return Some(slice.get(voxel.x as u32, voxel.y as u32, depth));
        }
    }

    None
}

/// Samples material at a voxel coordinate, handling neighbor lookups.
fn sample_material(
    voxel: IVec3,
    field: &MaterialField,
    neighbors: Option<&NeighborMaterialFields>,
) -> u8 {
    let size = FIELD_SIZE.as_ivec3();

    // In bounds - direct sample
    if voxel.x >= 0 && voxel.y >= 0 && voxel.z >= 0 
        && voxel.x < size.x && voxel.y < size.y && voxel.z < size.z 
    {
        return field.get(voxel.x as u32, voxel.y as u32, voxel.z as u32);
    }

    // Try neighbor lookup
    if let Some(neighbors) = neighbors {
        if let Some(material) = sample_material_neighbor(voxel, neighbors) {
            return material;
        }
    }

    // Out of bounds - return default material
    0
}

/// Samples material from neighbor fields.
fn sample_material_neighbor(voxel: IVec3, neighbors: &NeighborMaterialFields) -> Option<u8> {
    let size = FIELD_SIZE.as_ivec3();

    // -X neighbor
    if voxel.x < 0 && voxel.y >= 0 && voxel.z >= 0 && voxel.y < size.y && voxel.z < size.z {
        if let Some(ref slice) = neighbors.neighbors[NeighborFace::NegX as usize] {
            let depth = (-1 - voxel.x) as u32;
            return Some(slice.get(voxel.y as u32, voxel.z as u32, depth));
        }
    }

    // +X neighbor
    if voxel.x >= size.x && voxel.y >= 0 && voxel.z >= 0 && voxel.y < size.y && voxel.z < size.z {
        if let Some(ref slice) = neighbors.neighbors[NeighborFace::PosX as usize] {
            let depth = (voxel.x - size.x) as u32;
            return Some(slice.get(voxel.y as u32, voxel.z as u32, depth));
        }
    }

    // -Y neighbor
    if voxel.y < 0 && voxel.x >= 0 && voxel.z >= 0 && voxel.x < size.x && voxel.z < size.z {
        if let Some(ref slice) = neighbors.neighbors[NeighborFace::NegY as usize] {
            let depth = (-1 - voxel.y) as u32;
            return Some(slice.get(voxel.x as u32, voxel.z as u32, depth));
        }
    }

    // +Y neighbor
    if voxel.y >= size.y && voxel.x >= 0 && voxel.z >= 0 && voxel.x < size.x && voxel.z < size.z {
        if let Some(ref slice) = neighbors.neighbors[NeighborFace::PosY as usize] {
            let depth = (voxel.y - size.y) as u32;
            return Some(slice.get(voxel.x as u32, voxel.z as u32, depth));
        }
    }

    // -Z neighbor
    if voxel.z < 0 && voxel.x >= 0 && voxel.y >= 0 && voxel.x < size.x && voxel.y < size.y {
        if let Some(ref slice) = neighbors.neighbors[NeighborFace::NegZ as usize] {
            let depth = (-1 - voxel.z) as u32;
            return Some(slice.get(voxel.x as u32, voxel.y as u32, depth));
        }
    }

    // +Z neighbor
    if voxel.z >= size.z && voxel.x >= 0 && voxel.y >= 0 && voxel.x < size.x && voxel.y < size.y {
        if let Some(ref slice) = neighbors.neighbors[NeighborFace::PosZ as usize] {
            let depth = (voxel.z - size.z) as u32;
            return Some(slice.get(voxel.x as u32, voxel.y as u32, depth));
        }
    }

    None
}

/// Merges duplicate materials and normalizes weights to sum to 1.0.
fn merge_and_normalize_materials(contributions: &mut Vec<(u8, f32)>) {
    // Sort by material ID to group duplicates
    contributions.sort_by_key(|(mat, _)| *mat);

    // Merge duplicates by summing weights
    let mut merged: Vec<(u8, f32)> = Vec::with_capacity(contributions.len());
    for (mat, weight) in contributions.iter() {
        if let Some((last_mat, last_weight)) = merged.last_mut() {
            if *last_mat == *mat {
                *last_weight += weight;
                continue;
            }
        }
        merged.push((*mat, *weight));
    }

    // Sort by weight descending to keep top 4
    merged.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Normalize
    let sum: f32 = merged.iter().map(|(_, w)| w).sum();
    if sum > 0.0 {
        for (_, weight) in &mut merged {
            *weight /= sum;
        }
    }

    *contributions = merged;
}

/// Converts material contributions to VertexMaterialData.
fn contributions_to_vertex_data(contributions: &[(u8, f32)]) -> VertexMaterialData {
    match contributions.len() {
        0 => VertexMaterialData::single(0),
        1 => VertexMaterialData::single(contributions[0].0),
        2 => VertexMaterialData::blend2(
            contributions[0].0,
            contributions[1].0,
            contributions[1].1,
        ),
        3 => VertexMaterialData::blend3(
            contributions[0].0,
            contributions[1].0,
            contributions[2].0,
            contributions[0].1,
            contributions[1].1,
            contributions[2].1,
        ),
        _ => {
            // Take top 4
            let ids = [
                contributions[0].0,
                contributions[1].0,
                contributions[2].0,
                contributions[3].0,
            ];
            let weights = [
                contributions[0].1,
                contributions[1].1,
                contributions[2].1,
                contributions[3].1,
            ];
            VertexMaterialData::blend4(ids, weights)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_materials() {
        let mut contributions = vec![(1, 0.3), (2, 0.2), (1, 0.4), (3, 0.1)];
        merge_and_normalize_materials(&mut contributions);

        // Material 1 should be merged (0.3 + 0.4 = 0.7)
        // Should be sorted by weight descending
        assert_eq!(contributions[0].0, 1);
        assert!((contributions[0].1 - 0.7).abs() < 0.01);
    }

    #[test]
    fn test_contributions_to_vertex_data() {
        let data = contributions_to_vertex_data(&[(5, 1.0)]);
        assert_eq!(data.ids[0], 5);
        assert_eq!(data.weights[0], 255);

        let data = contributions_to_vertex_data(&[(1, 0.5), (2, 0.5)]);
        assert_eq!(data.ids[0], 1);
        assert_eq!(data.ids[1], 2);
    }
}
