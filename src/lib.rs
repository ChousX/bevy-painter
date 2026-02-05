//! # bevy_triplanar_voxel
//!
//! A Bevy plugin for rendering voxel terrain with triplanar texture mapping
//! and multi-material blending.
//!
//! ## Features
//!
//! - **Triplanar mapping**: No UV coordinates needed - textures projected from world space
//! - **Multi-material blending**: Up to 4 materials blended per vertex
//! - **Texture arrays**: Efficient GPU texture atlas for material palettes
//! - **PBR support**: Optional normal and ARM (AO/Roughness/Metallic) maps
//! - **Per-material properties**: Individual texture scale and blend sharpness

pub mod material;
#[cfg(feature = "material_field")]
pub mod material_field;
pub mod mesh;
pub mod palette;
mod plugin;

pub use plugin::TriplanarVoxelPlugin;

/// Prelude module with commonly used types.
pub mod prelude {
    // src/prelude.rs
    pub use crate::TriplanarVoxelPlugin;
    pub use crate::material::{TriplanarExtension, TriplanarSettings, TriplanarVoxelMaterial};
    pub use crate::mesh::{
        ATTRIBUTE_MATERIAL_IDS, ATTRIBUTE_MATERIAL_WEIGHTS, MeshTriplanarExt, TriplanarMeshBuilder,
        VertexMaterialData,
    };
    pub use crate::palette::{MAX_MATERIALS, MaterialPropertiesGpu};

    #[cfg(feature = "material_field")]
    pub use crate::material_field::{
        BoxLayer,
        ChunkBounds,
        FillLayer,
        HeightLayer,
        MaterialBlendSettings,
        // Core types
        MaterialField,
        MaterialFieldBundle,
        // Generation
        MaterialLayer,
        MaterialLayerStack,
        MaterialSlice,
        MaterialSliceExt,
        // Integration
        MeshMaterialIntegration,
        NeighborMaterialFields,
        SamplerLayer,
        SphereLayer,
    };
}
/// Shader asset path (embedded).
const TRIPLANAR_SHADER_PATH: &str =
    "embedded://bevy-painter/material/shaders/triplanar_extension.wgsl";
