//! Mesh utilities for triplanar voxel rendering.
//!
//! Provides custom vertex attributes and builders for creating meshes
//! compatible with the triplanar material system.

mod attributes;
mod builder;
mod vertex_data;

pub use attributes::{ATTRIBUTE_MATERIAL_IDS, ATTRIBUTE_MATERIAL_WEIGHTS};
pub use builder::TriplanarMeshBuilder;
pub use vertex_data::VertexMaterialData;
