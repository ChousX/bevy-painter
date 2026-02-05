//! Material field storage and blending for per-voxel material IDs.

mod blending;
mod field;
mod generation;
mod integration;

pub use blending::{MaterialBlendSettings, compute_vertex_materials};
pub use field::{FIELD_SIZE, FIELD_VOLUME, MaterialField, MaterialFieldDirty};
pub use generation::{
    BoxLayer, ChunkBounds, FillLayer, HeightLayer, MaterialLayer, MaterialLayerStack, SamplerLayer,
    SphereLayer,
};
pub use integration::{MaterialFieldBundle, MeshMaterialIntegration};

// Re-export neighbor types
pub use bevy_sculpter::neighbor::{NEIGHBOR_DEPTH, NeighborFace, NeighborFields, NeighborSlice};

pub type MaterialSlice = NeighborSlice<u8>;
pub type NeighborMaterialFields = NeighborFields<u8>;

/// Extension trait for creating material slices.
pub trait MaterialSliceExt {
    fn from_material_field(field: &MaterialField, face: NeighborFace) -> Self;
}

impl MaterialSliceExt for MaterialSlice {
    fn from_material_field(field: &MaterialField, face: NeighborFace) -> Self {
        Self::from_field(field, face)
    }
}
