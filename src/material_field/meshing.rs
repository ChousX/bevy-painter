//! Vertex material computation from voxel materials and density.

use bevy::prelude::*;

use crate::density_field::DensityField;
use crate::neighbor::NeighborDensityFields;
use super::{MaterialField, NeighborMaterialFields};

/// Maximum materials to blend per vertex.
pub const MAX_VERTEX_MATERIALS: usize = 4;

/// Computed material data for a single vertex.
#[derive(Clone, Copy, Debug, Default)]
pub struct VertexMaterial {
    /// Material IDs (up to 4).
    pub ids: [u8; 4],
    /// Blend weights (sum to 255).
    pub weights: [u8; 4],
}

impl VertexMaterial {
    /// Creates single-material vertex data.
    pub const fn single(id: u8) -> Self {
        Self {
            ids: [id, 0, 0, 0],
            weights: [255, 0, 0, 0],
        }
    }

    /// Packs material IDs into u32 for vertex attribute.
    #[inline]
    pub fn pack_ids(&self) -> u32 {
        u32::from_le_bytes(self.ids)
    }

    /// Packs weights into u32 for vertex attribute.
    #[inline]
    pub fn pack_weights(&self) -> u32 {
        u32::from_le_bytes(self.weights)
    }
}

/// Configuration for vertex material computation.
#[derive(Clone, Debug)]
pub struct VertexMaterialComputer {
    /// Minimum weight threshold (0-255). Materials below this are discarded.
    pub min_weight: u8,
    /// Whether to use density magnitude for weighting (vs binary inside/outside).
    pub density_weighted: bool,
    /// Exponent for density-based weighting. Higher = sharper transitions.
    pub density_power: f32,
}

impl Default for VertexMaterialComputer {
    fn default() -> Self {
        Self {
            min_weight: 5,
            density_weighted: true,
            density_power: 1.0,
        }
    }
}

impl VertexMaterialComputer {
    /// Computes vertex material from the 8 corners of a voxel cell.
    ///
    /// # Arguments
    /// * `cell` - Grid coordinates of the cell (0-31)
    /// * `density` - Density field
    /// * `materials` - Material field
    /// * `density_neighbors` - Optional neighbor density data
    /// * `material_neighbors` - Optional neighbor material data
    pub fn compute(
        &self,
        cell: IVec3,
        density: &DensityField,
        materials: &MaterialField,
        density_neighbors: Option<&NeighborDensityFields>,
        material_neighbors: Option<&NeighborMaterialFields>,
    ) -> VertexMaterial {
        // Sample 8 corners of the cell
        let mut contributions: [(u8, f32); 8] = [(0, 0.0); 8];
        let mut total_weight = 0.0f32;

        for (i, offset) in CUBE_CORNERS.iter().enumerate() {
            let pos = cell + *offset;

            // Get density (with neighbor fallback)
            let d = self.get_density(pos, density, density_neighbors);

            // Get material (with neighbor fallback)  
            let mat = self.get_material(pos, materials, material_neighbors);

            // Compute weight from density
            let weight = self.density_to_weight(d);

            contributions[i] = (mat, weight);
            total_weight += weight;
        }

        if total_weight < 0.0001 {
            // All corners outside surface - use first corner's material
            return VertexMaterial::single(contributions[0].0);
        }

        // Aggregate by material ID
        let mut aggregated: [(u8, f32); MAX_VERTEX_MATERIALS] = [(0, 0.0); MAX_VERTEX_MATERIALS];
        let mut agg_count = 0usize;

        for (mat, weight) in contributions {
            if weight < 0.0001 {
                continue;
            }

            // Find or insert material
            let mut found = false;
            for (existing_mat, existing_weight) in aggregated.iter_mut().take(agg_count) {
                if *existing_mat == mat {
                    *existing_weight += weight;
                    found = true;
                    break;
                }
            }

            if !found && agg_count < MAX_VERTEX_MATERIALS {
                aggregated[agg_count] = (mat, weight);
                agg_count += 1;
            }
        }

        // Sort by weight (descending) and take top 4
        aggregated[..agg_count].sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        // Normalize to 255
        let top_weight: f32 = aggregated.iter().take(4).map(|(_, w)| w).sum();
        
        if top_weight < 0.0001 {
            return VertexMaterial::single(aggregated[0].0);
        }

        let mut result = VertexMaterial::default();
        let mut running_total = 0u8;

        for (i, (mat, weight)) in aggregated.iter().take(4).enumerate() {
            result.ids[i] = *mat;

            if i == 3 {
                // Last slot absorbs rounding error
                result.weights[i] = 255 - running_total;
            } else {
                let normalized = ((weight / top_weight) * 255.0).round() as u8;
                let clamped = normalized.min(255 - running_total);
                result.weights[i] = clamped;
                running_total += clamped;
            }
        }

        // Filter out weights below threshold
        if self.min_weight > 0 {
            result = self.filter_low_weights(result);
        }

        result
    }

    fn get_density(
        &self,
        pos: IVec3,
        density: &DensityField,
        neighbors: Option<&NeighborDensityFields>,
    ) -> f32 {
        if let Some(d) = density.get_signed(pos.x, pos.y, pos.z) {
            return d;
        }

        if let Some(n) = neighbors {
            if let Some(d) = n.get_extended(density, pos.x, pos.y, pos.z) {
                return d;
            }
        }

        // Out of bounds - assume outside surface
        1.0
    }

    fn get_material(
        &self,
        pos: IVec3,
        materials: &MaterialField,
        neighbors: Option<&NeighborMaterialFields>,
    ) -> u8 {
        if let Some(m) = materials.get_signed(pos.x, pos.y, pos.z) {
            return m;
        }

        if let Some(n) = neighbors {
            if let Some(m) = n.get_extended(materials, pos.x, pos.y, pos.z) {
                return m;
            }
        }

        // Out of bounds - use default
        super::field::DEFAULT_MATERIAL
    }

    fn density_to_weight(&self, density: f32) -> f32 {
        if self.density_weighted {
            // Negative density = inside surface = positive weight
            // Apply power for sharper/softer transitions
            (-density).max(0.0).powf(self.density_power)
        } else {
            // Binary: inside = 1, outside = 0
            if density < 0.0 { 1.0 } else { 0.0 }
        }
    }

    fn filter_low_weights(&self, mut vm: VertexMaterial) -> VertexMaterial {
        // Zero out weights below threshold
        for w in vm.weights.iter_mut() {
            if *w < self.min_weight {
                *w = 0;
            }
        }

        // Renormalize remaining weights
        let sum: u16 = vm.weights.iter().map(|&w| w as u16).sum();
        if sum == 0 {
            // All filtered - keep highest original
            vm.weights[0] = 255;
            return vm;
        }

        if sum == 255 {
            return vm;
        }

        // Scale up to 255
        let scale = 255.0 / sum as f32;
        let mut running = 0u8;
        let mut last_nonzero = 0usize;

        for (i, w) in vm.weights.iter_mut().enumerate() {
            if *w > 0 {
                *w = ((*w as f32) * scale).round() as u8;
                running = running.saturating_add(*w);
                last_nonzero = i;
            }
        }

        // Fix rounding error
        if running != 255 {
            let diff = 255i16 - running as i16;
            vm.weights[last_nonzero] = (vm.weights[last_nonzero] as i16 + diff).max(0) as u8;
        }

        vm
    }
}

/// Cube corner offsets for sampling 8 voxels around a cell.
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

/// Convenience function to compute materials for all vertices in a mesh.
///
/// Call this after generating a surface nets mesh to add material attributes.
///
/// # Arguments
/// * `positions` - Vertex positions in grid space (0-32)
/// * `density` - The density field
/// * `materials` - The material field
/// * `density_neighbors` - Optional neighbor density data
/// * `material_neighbors` - Optional neighbor material data
/// * `mesh_size` - World size of the mesh (for grid-to-world conversion)
///
/// # Returns
/// Tuple of (material_ids, material_weights) as packed u32 vectors
pub fn compute_vertex_materials(
    positions: &[[f32; 3]],
    density: &DensityField,
    materials: &MaterialField,
    density_neighbors: Option<&NeighborDensityFields>,
    material_neighbors: Option<&NeighborMaterialFields>,
    mesh_size: Vec3,
) -> (Vec<u32>, Vec<u32>) {
    let computer = VertexMaterialComputer::default();
    let scale = crate::DENSITY_FIELD_SIZE.as_vec3() / mesh_size;

    let mut ids = Vec::with_capacity(positions.len());
    let mut weights = Vec::with_capacity(positions.len());

    for pos in positions {
        // Convert vertex position to grid cell
        let grid_pos = Vec3::from(*pos) * scale;
        let cell = grid_pos.floor().as_ivec3();

        let vm = computer.compute(
            cell,
            density,
            materials,
            density_neighbors,
            material_neighbors,
        );

        ids.push(vm.pack_ids());
        weights.push(vm.pack_weights());
    }

    (ids, weights)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_material() {
        let density = DensityField::new(); // All positive (outside)
        let materials = MaterialField::filled(5);
        let computer = VertexMaterialComputer::default();

        let vm = computer.compute(
            IVec3::new(16, 16, 16),
            &density,
            &materials,
            None,
            None,
        );

        assert_eq!(vm.ids[0], 5);
    }

    #[test]
    fn test_weight_normalization() {
        let vm = VertexMaterial {
            ids: [0, 1, 2, 3],
            weights: [100, 100, 55, 0],
        };

        let sum: u16 = vm.weights.iter().map(|&w| w as u16).sum();
        assert_eq!(sum, 255);
    }
}
