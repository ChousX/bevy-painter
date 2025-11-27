//! Systems for managing triplanar materials.

use crate::palette::TexturePalette;
use bevy::prelude::*;
use std::collections::HashSet;

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
                bevy::prelude::info!(
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

/// Marker component for entities that need palette validation.
#[derive(Component)]
pub struct NeedsPaletteValidation;

#[cfg(test)]
mod tests {
    use super::*;

    // Integration tests would go here, but require a full app context
}
