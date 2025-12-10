use bevy::app::{App, Plugin};
use bevy::pbr::MaterialPlugin;

use crate::material::TriplanarVoxelMaterial;

/// Plugin that registers the triplanar voxel material system.
pub struct TriplanarVoxelPlugin;

impl Plugin for TriplanarVoxelPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<TriplanarVoxelMaterial>::default());
    }
}
