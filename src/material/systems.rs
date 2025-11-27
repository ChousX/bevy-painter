//! Systems for managing triplanar materials.

use bevy::prelude::*;
use std::collections::HashSet;

use super::extension::TriplanarSettings;
use crate::palette::TexturePalette;

/// System set for triplanar material systems.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct TriplanarMaterialSystems;

/// System that validates palettes when they are loaded or changed.
///
/// This system runs in the `Update` schedule and checks that all
/// texture palettes have valid formats and dimensions.
///
/// # Panics
///
/// Panics if a palette fails validation. This is intentional to catch
/// asset configuration errors early in development.
pub fn validate_palettes(
    palettes: Res<Assets<TexturePalette>>,
    images: Res<Assets<Image>>,
    mut validated: Local<HashSet<AssetId<TexturePalette>>>,
    mut events: MessageReader<AssetEvent<TexturePalette>>,
) {
    for event in events.read() {
        match event {
            AssetEvent::Added { id } | AssetEvent::Modified { id } => {
                // Mark for validation
                validated.remove(id);
            }
            AssetEvent::Removed { id } => {
                validated.remove(id);
            }
            _ => {}
        }
    }

    for (id, palette) in palettes.iter() {
        if validated.contains(&id) {
            continue;
        }

        // Check if all required images are loaded
        let albedo_loaded = images.contains(&palette.albedo);
        let normal_loaded = palette
            .normal
            .as_ref()
            .map(|h| images.contains(h))
            .unwrap_or(true);
        let arm_loaded = palette
            .arm
            .as_ref()
            .map(|h| images.contains(h))
            .unwrap_or(true);

        if !albedo_loaded || !normal_loaded || !arm_loaded {
            // Not all assets loaded yet, skip validation
            continue;
        }

        // Validate the palette
        match palette.validate(&images) {
            Ok(()) => {
                info!(
                    "Validated texture palette with {} materials",
                    palette.material_count()
                );
                validated.insert(id);
            }
            Err(e) => {
                panic!("Texture palette validation failed: {}", e);
            }
        }
    }
}

/// System that updates material settings when their palettes change.
///
/// This ensures that material flags (like `has_arm`) are kept in sync
/// with the actual palette contents.
pub fn sync_material_settings(
    palettes: Res<Assets<TexturePalette>>,
    mut materials: ResMut<Assets<crate::material::TriplanarVoxelMaterial>>,
) {
    if !palettes.is_changed() {
        return;
    }

    for (_, material) in materials.iter_mut() {
        let ext = &mut material.extension;

        if let Some(palette) = palettes.get(&ext.palette) {
            // Update ARM flag based on palette
            let has_arm = palette.has_arm();
            if has_arm {
                ext.settings.flags |= TriplanarSettings::FLAG_HAS_ARM;
            } else {
                ext.settings.flags &= !TriplanarSettings::FLAG_HAS_ARM;
            }

            // Update normal flag based on palette and setting
            let can_use_normals = ext.enable_normal_maps && palette.has_normal_maps();
            if can_use_normals {
                ext.settings.flags |= TriplanarSettings::FLAG_ENABLE_NORMALS;
            } else {
                ext.settings.flags &= !TriplanarSettings::FLAG_ENABLE_NORMALS;
            }
        }
    }
}

/// Marker component for entities that need palette validation.
#[derive(Component)]
pub struct NeedsPaletteValidation;

#[cfg(test)]
mod tests {
    use super::*;

    // Integration tests would go here, but require a full app context
}
