//! Material and visibility blending logic based on density values.

use bevy::prelude::*;
use bevy_sculpter::{field::Field, prelude::NeighborDensityFields};

use bevy_sculpter::prelude::DefaultIsoField as DensityField;

use super::visibility::{NeighborVisibilityFields, VisibilityField};
use super::{MaterialField, NeighborMaterialFields};
use crate::mesh::VertexMaterialData;

/// Settings for material blending at vertices.
#[derive(Resource, Clone, Debug)]
pub struct MaterialBlendSettings {
    /// How much negative density contributes to material weight.
    pub density_influence: f32,
    /// Minimum weight threshold to include a material in blending.
    pub weight_threshold: f32,
    /// Visibility threshold - vertices below this are discarded.
    /// Default: 0.5 (50% of surrounding voxels must be visible)
    pub visibility_threshold: f32,
}

impl Default for MaterialBlendSettings {
    fn default() -> Self {
        Self {
            density_influence: 2.0,
            weight_threshold: 0.01,
            visibility_threshold: 0.5,
        }
    }
}

/// Result of computing vertex material and visibility data.
#[derive(Clone, Debug)]
pub struct VertexBlendResult {
    pub material_data: VertexMaterialData,
    /// Visibility value from 0.0 (hidden) to 1.0 (fully visible)
    pub visibility: f32,
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

/// Computes material blend data for a vertex (original function, no visibility).
pub fn compute_vertex_materials(
    world_pos: Vec3,
    mesh_size: Vec3,
    density_field: &DensityField,
    material_field: &MaterialField,
    neighbor_densities: Option<&NeighborDensityFields>,
    neighbor_materials: Option<&NeighborMaterialFields>,
    settings: &MaterialBlendSettings,
) -> VertexMaterialData {
    compute_vertex_blend(
        world_pos,
        mesh_size,
        density_field,
        material_field,
        None,
        neighbor_densities,
        neighbor_materials,
        None,
        settings,
    )
    .material_data
}

/// Computes material AND visibility data for a vertex.
pub fn compute_vertex_blend(
    world_pos: Vec3,
    mesh_size: Vec3,
    density_field: &DensityField,
    material_field: &MaterialField,
    visibility_field: Option<&VisibilityField>,
    neighbor_densities: Option<&NeighborDensityFields>,
    neighbor_materials: Option<&NeighborMaterialFields>,
    neighbor_visibility: Option<&NeighborVisibilityFields>,
    settings: &MaterialBlendSettings,
) -> VertexBlendResult {
    let field_size = DensityField::SIZE;
    let scale = field_size.as_vec3() / mesh_size;
    let grid_pos = world_pos * scale;
    let base = grid_pos.floor().as_ivec3();

    let mut contributions: Vec<(u8, f32)> = Vec::with_capacity(8);
    let mut visibility_sum: f32 = 0.0;
    let mut visibility_weight_sum: f32 = 0.0;

    let mut any_valid_sample = false;
    let mut fallback_material: u8 = 0;

    for offset in &CORNER_OFFSETS {
        let voxel = base + *offset;

        let Some((density, material)) = sample_voxel_material(
            voxel,
            density_field,
            material_field,
            neighbor_densities,
            neighbor_materials,
        ) else {
            continue;
        };

        // Sample visibility (defaults to visible if no visibility field)
        let vis = sample_visibility(voxel, visibility_field, neighbor_visibility);

        if !any_valid_sample {
            any_valid_sample = true;
            fallback_material = material;
        }

        // Interior voxels contribute to material blending
        if density < 0.0 {
            let weight = (-density * settings.density_influence).clamp(0.0, 1.0);
            if weight > settings.weight_threshold {
                contributions.push((material, weight));
                // Weight visibility by density contribution
                visibility_sum += vis * weight;
                visibility_weight_sum += weight;
            }
        }
    }

    // Compute final visibility
    let visibility = if visibility_weight_sum > 0.0 {
        visibility_sum / visibility_weight_sum
    } else {
        // No interior voxels - use average of all sampled visibility
        1.0 // Default visible if no data
    };

    // Material fallback logic (same as before)
    let material_data = if contributions.is_empty() {
        if any_valid_sample {
            VertexMaterialData::single(fallback_material)
        } else {
            let field_size_i = field_size.as_ivec3();
            let clamped = grid_pos
                .round()
                .as_ivec3()
                .clamp(IVec3::ZERO, field_size_i - IVec3::ONE);
            let material = material_field.get(clamped.x as u32, clamped.y as u32, clamped.z as u32);
            VertexMaterialData::single(material)
        }
    } else {
        merge_and_normalize_materials(&mut contributions);
        contributions_to_vertex_data(&contributions)
    };

    VertexBlendResult {
        material_data,
        visibility,
    }
}

/// Samples visibility at a voxel coordinate.
/// Returns 1.0 if no visibility field is provided (default visible).
fn sample_visibility(
    voxel: IVec3,
    visibility_field: Option<&VisibilityField>,
    neighbor_visibility: Option<&NeighborVisibilityFields>,
) -> f32 {
    let Some(vis_field) = visibility_field else {
        return 1.0; // No visibility field = fully visible
    };

    // Try local field first
    if let Some(vis) = vis_field.get_ivec3(voxel) {
        return vis as f32 / 255.0;
    }

    // Try neighbor data
    if let Some(neighbors) = neighbor_visibility {
        if let Some(vis) = neighbors.sample_for::<VisibilityField>(voxel) {
            return vis as f32 / 255.0;
        }
    }

    // Out of bounds with no neighbor = hidden
    0.0
}

/// Samples both density and material at a voxel coordinate.
#[inline]
fn sample_voxel_material(
    voxel: IVec3,
    density_field: &DensityField,
    material_field: &MaterialField,
    neighbor_densities: Option<&NeighborDensityFields>,
    neighbor_materials: Option<&NeighborMaterialFields>,
) -> Option<(f32, u8)> {
    if let (Some(density), Some(material)) = (
        density_field.get_ivec3(voxel),
        material_field.get_ivec3(voxel),
    ) {
        return Some((density, material));
    }

    let density = neighbor_densities?.sample_for::<DensityField>(voxel)?;
    let material = neighbor_materials?.sample_for::<MaterialField>(voxel)?;

    Some((density, material))
}

fn merge_and_normalize_materials(contributions: &mut Vec<(u8, f32)>) {
    contributions.sort_by_key(|(mat, _)| *mat);

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

    merged.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let sum: f32 = merged.iter().map(|(_, w)| w).sum();
    if sum > 0.0 {
        for (_, weight) in &mut merged {
            *weight /= sum;
        }
    }

    *contributions = merged;
}

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
