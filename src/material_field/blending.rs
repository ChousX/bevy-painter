//! Material blending logic based on density values.

use bevy::prelude::*;
use bevy_sculpter::prelude::{DensityField, NeighborDensityFields};

use super::{FIELD_SIZE, MaterialField, NeighborMaterialFields};
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
const CORNER_OFFSETS: [IVec3; 8] = [
    IVec3::new(0, 0, 0),
    IVec3::new(1, 0, 0),
    IVec3::new(0, 1, 0),
    IVec3::new(1, 1, 0),
    IVec3::new(0, 0, 1),
    IVec3::new(1, 0, 1),
    IVec3::new(0, 1, 1),
    IVec3::new(1, 1, 1),
];

/// Computes material blend data for a vertex at the given world position.
///
/// Samples the 8 surrounding voxels and blends their materials based on
/// how "inside" each voxel is (negative density = inside).
///
/// Only contributes voxels where BOTH density and material data are available,
/// preventing incorrect material 0 blending at chunk boundaries.
pub fn compute_vertex_materials(
    world_pos: Vec3,
    mesh_size: Vec3,
    density_field: &DensityField,
    material_field: &MaterialField,
    neighbor_densities: Option<&NeighborDensityFields>,
    neighbor_materials: Option<&NeighborMaterialFields>,
    settings: &MaterialBlendSettings,
) -> VertexMaterialData {
    let scale = FIELD_SIZE.as_vec3() / mesh_size;
    let grid_pos = world_pos * scale;
    let base = grid_pos.floor().as_ivec3();
    let field_size = FIELD_SIZE.as_ivec3();

    // Collect materials and their weights from 8 surrounding voxels
    let mut contributions: Vec<(u8, f32)> = Vec::with_capacity(8);

    // Track if we got any valid samples for fallback
    let mut any_valid_sample = false;
    let mut fallback_material: u8 = 0;

    for offset in &CORNER_OFFSETS {
        let voxel = base + *offset;

        // Only contribute if we have BOTH valid density AND material
        let Some((density, material)) = sample_voxel(
            voxel,
            density_field,
            material_field,
            neighbor_densities,
            neighbor_materials,
            field_size,
        ) else {
            continue;
        };

        // Track for fallback
        if !any_valid_sample {
            any_valid_sample = true;
            fallback_material = material;
        }

        // Convert density to weight: more negative = more "inside" = higher weight
        // Only interior voxels (negative density) contribute
        if density < 0.0 {
            let weight = (-density * settings.density_influence).clamp(0.0, 1.0);
            if weight > settings.weight_threshold {
                contributions.push((material, weight));
            }
        }
    }

    // If no interior voxels contributed, use fallback
    if contributions.is_empty() {
        if any_valid_sample {
            return VertexMaterialData::single(fallback_material);
        }
        
        // Absolute fallback: sample nearest in-bounds voxel
        let clamped = grid_pos.round().as_ivec3().clamp(
            IVec3::ZERO,
            field_size - IVec3::ONE,
        );
        let material = material_field.get(
            clamped.x as u32,
            clamped.y as u32,
            clamped.z as u32,
        );
        return VertexMaterialData::single(material);
    }

    // Merge duplicate materials and normalize weights
    merge_and_normalize_materials(&mut contributions);

    // Convert to VertexMaterialData (up to 4 materials)
    contributions_to_vertex_data(&contributions)
}

/// Samples both density and material at a voxel coordinate.
/// Returns `None` if either value is unavailable (out of bounds with no neighbor data).
///
/// This ensures consistency - we only blend voxels where we have complete information.
#[inline]
fn sample_voxel(
    voxel: IVec3,
    density_field: &DensityField,
    material_field: &MaterialField,
    neighbor_densities: Option<&NeighborDensityFields>,
    neighbor_materials: Option<&NeighborMaterialFields>,
    field_size: IVec3,
) -> Option<(f32, u8)> {
    // Check if in bounds
    let in_bounds = voxel.x >= 0
        && voxel.y >= 0
        && voxel.z >= 0
        && voxel.x < field_size.x
        && voxel.y < field_size.y
        && voxel.z < field_size.z;

    if in_bounds {
        // Direct sample from local fields
        let density = density_field.get(voxel.x as u32, voxel.y as u32, voxel.z as u32);
        let material = material_field.get(voxel.x as u32, voxel.y as u32, voxel.z as u32);
        return Some((density, material));
    }

    // Out of bounds - need BOTH neighbor fields to have data
    let density = neighbor_densities?.sample(voxel, field_size)?;
    let material = neighbor_materials?.sample(voxel, field_size)?;

    Some((density, material))
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
        2 => VertexMaterialData::blend2(contributions[0].0, contributions[1].0, contributions[1].1),
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

    #[test]
    fn test_sample_voxel_in_bounds() {
        let mut density_field = DensityField::new();
        let mut material_field = MaterialField::new();
        
        density_field.set(5, 5, 5, -0.5);
        material_field.set(5, 5, 5, 3);
        
        let result = sample_voxel(
            IVec3::new(5, 5, 5),
            &density_field,
            &material_field,
            None,
            None,
            FIELD_SIZE.as_ivec3(),
        );
        
        assert_eq!(result, Some((-0.5, 3)));
    }

    #[test]
    fn test_sample_voxel_out_of_bounds_no_neighbors() {
        let density_field = DensityField::new();
        let material_field = MaterialField::new();
        
        // Out of bounds with no neighbor data should return None
        let result = sample_voxel(
            IVec3::new(-1, 5, 5),
            &density_field,
            &material_field,
            None,
            None,
            FIELD_SIZE.as_ivec3(),
        );
        
        assert_eq!(result, None);
    }
}
