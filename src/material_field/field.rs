//! Material field storage for per-voxel material IDs.

use bevy::prelude::*;
use bevy_sculpter::field::Field;

/// Size of the material field grid (must match bevy_sculpter::DENSITY_FIELD_SIZE).
pub const FIELD_SIZE: UVec3 = uvec3(32, 32, 32);

/// Total number of voxels in the field.
pub const FIELD_VOLUME: usize = (FIELD_SIZE.x * FIELD_SIZE.y * FIELD_SIZE.z) as usize;

/// A 3D grid of material IDs for voxel terrain.
///
/// Each voxel stores a `u8` material index that references a layer in the
/// texture palette. Materials are blended at vertices based on the surrounding
/// voxels' density values from `bevy_sculpter::DensityField`.
///
/// # Coordinate System
///
/// Uses the same X-Y-Z ordering as `DensityField` (X varies fastest).
///
/// # Example
///
/// ```
/// use bevy_painter::material_field::MaterialField;
///
/// let mut field = MaterialField::new();
///
/// // Set material at a specific voxel
/// field.set(16, 16, 16, 2); // Material index 2
///
/// // Query material
/// assert_eq!(field.get(16, 16, 16), 2);
/// ```
#[derive(Component, Clone, Debug)]
pub struct MaterialField(pub Vec<u8>);

impl Default for MaterialField {
    fn default() -> Self {
        Self(vec![0; FIELD_VOLUME])
    }
}

impl Field<u8> for MaterialField {
    const SIZE: UVec3 = FIELD_SIZE;
    const DEFAULT: u8 = 0;

    #[inline]
    fn data(&self) -> &[u8] {
        &self.0
    }

    #[inline]
    fn data_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

impl MaterialField {
    /// Creates a new material field with all voxels set to material 0.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a material field with all voxels set to the given material ID.
    pub fn filled(material_id: u8) -> Self {
        Self(vec![material_id; FIELD_VOLUME])
    }

    // =========================================================================
    // Material-specific convenience methods
    // =========================================================================

    /// Paints a spherical region with a material.
    ///
    /// This is a convenience wrapper around [`FieldSphereOps::fill_sphere`].
    pub fn paint_sphere(&mut self, center: IVec3, radius: i32, material_id: u8) {
        use bevy_sculpter::field::FieldSphereOps;
        self.fill_sphere(center.as_vec3(), radius as f32, material_id);
    }

    /// Paints a box region with a material.
    ///
    /// This is a convenience wrapper around [`FieldBoxOps::fill_box`].
    pub fn paint_box(&mut self, min: IVec3, max: IVec3, material_id: u8) {
        use bevy_sculpter::field::FieldBoxOps;
        self.fill_box(min, max, material_id);
    }

    /// Paints materials based on height (Y coordinate).
    ///
    /// Useful for basic terrain layering (e.g., grass on top, dirt below, stone at bottom).
    ///
    /// # Arguments
    /// * `layers` - Slice of (max_height, material_id) pairs, processed bottom to top
    pub fn paint_height_layers(&mut self, layers: &[(u32, u8)]) {
        for pos in Self::positions() {
            let material = layers
                .iter()
                .find(|(max_y, _)| pos.y < *max_y)
                .map(|(_, mat)| *mat)
                .unwrap_or(0);
            self.set(pos.x, pos.y, pos.z, material);
        }
    }

    /// Paints materials based on a 3D sampling function.
    ///
    /// # Arguments
    /// * `sampler` - Function that takes grid coordinates and returns a material ID
    pub fn paint_with<F>(&mut self, sampler: F)
    where
        F: Fn(UVec3) -> u8,
    {
        for pos in Self::positions() {
            self.set(pos.x, pos.y, pos.z, sampler(pos));
        }
    }
}

/// Marker component indicating this chunk's material field needs processing.
#[derive(Component, Clone, Copy, Default, Debug)]
pub struct MaterialFieldDirty;

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_sculpter::field::{FieldBoxOps, FieldSphereOps};

    #[test]
    fn test_new_field() {
        let field = MaterialField::new();
        assert_eq!(field.0.len(), FIELD_VOLUME);
        assert!(field.0.iter().all(|&m| m == 0));
    }

    #[test]
    fn test_filled() {
        let field = MaterialField::filled(5);
        assert!(field.0.iter().all(|&m| m == 5));
    }

    #[test]
    fn test_get_set() {
        let mut field = MaterialField::new();
        field.set(10, 15, 20, 42);
        assert_eq!(field.get(10, 15, 20), 42);
        assert_eq!(field.get(0, 0, 0), 0);
    }

    #[test]
    fn test_out_of_bounds() {
        let mut field = MaterialField::new();
        field.set(100, 100, 100, 99); // Should be ignored
        assert_eq!(field.get(100, 100, 100), 0); // Returns default
    }

    #[test]
    fn test_paint_sphere_via_trait() {
        let mut field = MaterialField::new();
        field.fill_sphere(vec3(16.0, 16.0, 16.0), 3.0, 7);

        // Center should be painted
        assert_eq!(field.get(16, 16, 16), 7);
        // Just outside should not be painted
        assert_eq!(field.get(16, 16, 20), 0);
    }

    #[test]
    fn test_iter() {
        let field = MaterialField::new();
        assert_eq!(field.iter().count(), FIELD_VOLUME);
    }
}
