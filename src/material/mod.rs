//! Material extension for triplanar voxel rendering.
use bevy::prelude::*;
mod extension;

pub use extension::{TriplanarExtension, TriplanarSettings, TriplanarVoxelMaterial};

/// Register embedded shader assets for the material module.
pub(crate) fn register_embedded_assets(app: &mut App) {
bevy::asset::
    embedded_asset!(app, "shaders/triplanar_extension.wgsl");
}
