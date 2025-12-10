//! Triplanar voxel material implementation.
//!
//! This module provides [`TriplanarExtension`], a material extension that adds
//! triplanar mapping and multi-material blending to Bevy's [`StandardMaterial`].

mod extension;

pub use extension::{TriplanarExtension, TriplanarSettings, TriplanarVoxelMaterial};
