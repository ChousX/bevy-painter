//! Plugin for triplanar voxel materials.
use bevy::prelude::*;

use crate::material::TriplanarVoxelMaterial;

/// Plugin that adds triplanar voxel material support to Bevy.
///
/// This plugin registers:
/// - [`TriplanarVoxelMaterial`] as a material type
/// - Embedded shader assets
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
        // Embed the shader into the binary
        crate::material::register_embedded_assets(app);
        app
            // Register material (includes shader loading)
            .add_plugins(MaterialPlugin::<TriplanarVoxelMaterial>::default());
    }
}
