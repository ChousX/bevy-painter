//! Per-voxel material storage for terrain texturing.
//!
//! This module provides [`MaterialField`], a component that stores material IDs
//! parallel to [`DensityField`]. During mesh generation, materials are blended
//! at vertices based on neighboring voxel materials and density values.
//!
//! # Feature Flag
//!
//! This module is only available when the `material_field` feature is enabled:
//!
//! ```toml
//! bevy-sculpter = { version = "...", features = ["material_field"] }
//! ```
//!
//! # Overview
//!
//! Each voxel stores a single `u8` material ID. Blending between materials
//! happens automatically at surface vertices where different materials meet,
//! using density values to weight contributions.
//!
//! This approach excels at:
//! - Organic terrain with natural material transitions
//! - Procedural worlds with height/slope-based materials
//! - Destructible environments where carved surfaces reveal interior materials
//!
//! # Example
//!
//! ```ignore
//! use bevy_sculpter::prelude::*;
//! use bevy_sculpter::material_field::prelude::*;
//!
//! fn setup(mut commands: Commands) {
//!     let mut density = DensityField::new();
//!     let mut materials = MaterialField::new();
//!
//!     // Fill with terrain
//!     fill_centered_sphere(&mut density, 12.0);
//!
//!     // Paint materials by height
//!     materials.fill_by_height(|y| {
//!         if y > 20.0 { 2 }      // Snow
//!         else if y > 10.0 { 0 } // Grass  
//!         else { 1 }             // Stone
//!     });
//!
//!     commands.spawn((
//!         Chunk,
//!         ChunkPos(ivec3(0, 0, 0)),
//!         density,
//!         materials,
//!         DensityFieldDirty,
//!     ));
//! }
//! ```

mod field;
mod meshing;
mod neighbor;
mod brushes;

pub use field::MaterialField;
pub use meshing::{compute_vertex_materials, VertexMaterialComputer};
pub use neighbor::NeighborMaterialFields;
pub use brushes::{paint_sphere, paint_sphere_smooth};

/// Prelude for material field functionality.
pub mod prelude {
    pub use super::{
        MaterialField,
        NeighborMaterialFields,
        compute_vertex_materials,
        paint_sphere,
        paint_sphere_smooth,
    };
}
