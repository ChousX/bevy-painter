//! Plugin for triplanar voxel materials.

use bevy::prelude::*;

use crate::material::{validate_palettes, TriplanarMaterialSystems, TriplanarVoxelMaterial};
use crate::palette::TexturePalette;

/// Plugin that adds triplanar voxel material support to Bevy.
///
/// This plugin registers:
/// - [`TexturePalette`] as an asset type
/// - [`TriplanarVoxelMaterial`] as a material type
/// - Palette validation systems
///
/// # Example
/// ```ignore
/// use bevy::prelude::*;
/// use bevy_triplanar_voxel::TriplanarVoxelPlugin;
///
/// App::new()
///     .add_plugins(DefaultPlugins)
///     .add_plugins(TriplanarVoxelPlugin)
///     .run();
/// ```
pub struct TriplanarVoxelPlugin;

impl Plugin for TriplanarVoxelPlugin {
    fn build(&self, app: &mut App) {
        app
            // Register assets
            .init_asset::<TexturePalette>()
            // Register material
            .add_plugins(MaterialPlugin::<TriplanarVoxelMaterial>::default())
            // Add validation systems
            .add_systems(
                Update,
                validate_palettes.in_set(TriplanarMaterialSystems),
            );
    }
}
