//! Material field storage and blending for per-voxel material IDs.
//!
//! This module provides:
//! - [`MaterialField`]: Per-voxel material ID storage
//! - [`NeighborMaterialFields`]: Cached neighbor data for seamless boundaries
//! - Material blending logic for vertex attribute computation

mod blending;
mod field;

pub use blending::{MaterialBlendSettings, compute_vertex_materials};
pub use field::{FIELD_SIZE, FIELD_VOLUME, MaterialField, MaterialFieldDirty};

// Re-export neighbor types from bevy_sculpter with material-specific aliases
pub use bevy_sculpter::neighbor::{NEIGHBOR_DEPTH, NeighborFace, NeighborFields, NeighborSlice};

/// Neighbor slice for material field data (u8).
pub type MaterialSlice = NeighborSlice<u8>;

/// Cached neighbor material data for seamless meshing.
pub type NeighborMaterialFields = NeighborFields<u8>;

/// Extension trait for creating material slices from material fields.
pub trait MaterialSliceExt {
    /// Creates a material slice from a neighbor chunk's boundary planes.
    ///
    /// # Arguments
    /// * `field` - The neighbor's material field
    /// * `face` - Which face of the neighbor to sample
    fn from_material_field(field: &MaterialField, face: NeighborFace) -> Self;
}

impl MaterialSliceExt for MaterialSlice {
    fn from_material_field(field: &MaterialField, face: NeighborFace) -> Self {
        Self::from_sampler(face, FIELD_SIZE, |a, b, depth| {
            let (x, y, z) = face.to_field_coords(a, b, depth, FIELD_SIZE);
            field.get(x, y, z)
        })
    }
}
