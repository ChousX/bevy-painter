//! Triplanar voxel material implementation.
//!
//! This module provides [`TriplanarExtension`], a material extension that adds
//! triplanar mapping and multi-material blending to Bevy's [`StandardMaterial`].

mod extension;
mod systems;

pub use extension::{TriplanarExtension, TriplanarSettings, TriplanarVoxelMaterial};
pub use systems::{TriplanarMaterialSystems, validate_palettes};
