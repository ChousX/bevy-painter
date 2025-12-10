//! # bevy_painter
//!
//! A Bevy plugin for rendering voxel terrain with triplanar texture mapping
//! and multi-material blending.
//!
//! ## Features
//!
//! - Triplanar/biplanar texture projection (no UV seams)
//! - Up to 4 materials blended per vertex
//! - Texture array support for efficient material palettes
//! - Optional normal and ARM (AO/Roughness/Metallic) maps
//! - Per-material texture scaling and blend sharpness
//!
//! ## Quick Start
//!
//! ```ignore
//! use bevy::prelude::*;
//! use bevy_painter::prelude::*;
//!
//! fn main() {
//!     App::new()
//!         .add_plugins(DefaultPlugins)
//!         .add_plugins(TriplanarVoxelPlugin)
//!         .add_systems(Startup, setup)
//!         .run();
//! }
//!
//! fn setup(
//!     mut commands: Commands,
//!     mut meshes: ResMut<Assets<Mesh>>,
//!     mut materials: ResMut<Assets<TriplanarVoxelMaterial>>,
//!     asset_server: Res<AssetServer>,
//! ) {
//!     // Create a material extension with texture palette
//!     let extension = PaletteBuilder::new()
//!         .with_albedo(asset_server.load("terrain/albedo.ktx2"))
//!         .add_material_named("grass")
//!         .add_material_named("stone")
//!         .build();
//!
//!     // Create the material
//!     let material = TriplanarVoxelMaterial {
//!         base: StandardMaterial::default(),
//!         extension,
//!     };
//!
//!     // Spawn the mesh
//!     commands.spawn((
//!         Mesh3d(meshes.add(mesh)),
//!         MeshMaterial3d(materials.add(material)),
//!     ));
//! }
//! ```

pub mod material;
pub mod mesh;
pub mod palette;
mod plugin;

/// Per-voxel material storage for terrain texturing.
#[cfg(feature = "material_field")]
pub mod material_field;

pub mod prelude {
    pub use crate::material::{TriplanarExtension, TriplanarSettings, TriplanarVoxelMaterial};
    pub use crate::mesh::{
        MeshTriplanarExt, TriplanarMeshBuilder, VertexMaterialData, ATTRIBUTE_MATERIAL_IDS,
        ATTRIBUTE_MATERIAL_WEIGHTS,
    };
    pub use crate::palette::{MaterialPropertiesGpu, PaletteMaterial, PaletteBuilder, MAX_MATERIALS};
    pub use crate::plugin::TriplanarVoxelPlugin;

    #[cfg(feature = "material_field")]
    pub use crate::material_field::{
        add_material_attributes, add_material_attributes_with, fill_by_steepness,
        fill_height_layers, paint_sphere, paint_surface, BlendConfig, MaterialField,
    };
}
