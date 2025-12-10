//! Material field storage for per-voxel material IDs.

use bevy::prelude::*;

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

impl MaterialField {
    /// Creates a new material field with all voxels set to material 0.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a material field with all voxels set to the given material ID.
    pub fn filled(material_id: u8) -> Self {
        Self(vec![material_id; FIELD_VOLUME])
    }

    /// Computes the flat array index for a given (x, y, z) coordinate.
    #[inline]
    pub fn index(x: u32, y: u32, z: u32) -> usize {
        (x + y * FIELD_SIZE.x + z * FIELD_SIZE.x * FIELD_SIZE.y) as usize
    }

    /// Checks if coordinates are within bounds.
    #[inline]
    pub fn in_bounds(x: i32, y: i32, z: i32) -> bool {
        x >= 0
            && y >= 0
            && z >= 0
            && (x as u32) < FIELD_SIZE.x
            && (y as u32) < FIELD_SIZE.y
            && (z as u32) < FIELD_SIZE.z
    }

    /// Sets the material ID at the given coordinates.
    ///
    /// Silently ignores out-of-bounds coordinates.
    #[inline]
    pub fn set(&mut self, x: u32, y: u32, z: u32, material_id: u8) {
        if x < FIELD_SIZE.x && y < FIELD_SIZE.y && z < FIELD_SIZE.z {
            self.0[Self::index(x, y, z)] = material_id;
        }
    }

    /// Gets the material ID at the given coordinates.
    ///
    /// Returns `0` for out-of-bounds coordinates.
    #[inline]
    pub fn get(&self, x: u32, y: u32, z: u32) -> u8 {
        if x < FIELD_SIZE.x && y < FIELD_SIZE.y && z < FIELD_SIZE.z {
            self.0[Self::index(x, y, z)]
        } else {
            0
        }
    }

    /// Gets the material ID using signed coordinates.
    ///
    /// Returns `None` for out-of-bounds coordinates.
    #[inline]
    pub fn get_signed(&self, x: i32, y: i32, z: i32) -> Option<u8> {
        if Self::in_bounds(x, y, z) {
            Some(self.0[Self::index(x as u32, y as u32, z as u32)])
        } else {
            None
        }
    }

    /// Fills the entire field with a single material.
    pub fn fill(&mut self, material_id: u8) {
        self.0.fill(material_id);
    }

    /// Paints a spherical region with a material.
    ///
    /// # Arguments
    /// * `center` - Center of the sphere in grid coordinates
    /// * `radius` - Radius in grid units
    /// * `material_id` - Material to paint
    pub fn paint_sphere(&mut self, center: IVec3, radius: i32, material_id: u8) {
        let radius_sq = radius * radius;
        let min = (center - IVec3::splat(radius)).max(IVec3::ZERO);
        let max = (center + IVec3::splat(radius)).min(FIELD_SIZE.as_ivec3() - IVec3::ONE);

        for z in min.z..=max.z {
            for y in min.y..=max.y {
                for x in min.x..=max.x {
                    let pos = ivec3(x, y, z);
                    let dist_sq = (pos - center).length_squared();
                    if dist_sq <= radius_sq {
                        self.set(x as u32, y as u32, z as u32, material_id);
                    }
                }
            }
        }
    }

    /// Paints a box region with a material.
    ///
    /// # Arguments
    /// * `min` - Minimum corner (inclusive)
    /// * `max` - Maximum corner (inclusive)
    /// * `material_id` - Material to paint
    pub fn paint_box(&mut self, min: IVec3, max: IVec3, material_id: u8) {
        let min = min.max(IVec3::ZERO);
        let max = max.min(FIELD_SIZE.as_ivec3() - IVec3::ONE);

        for z in min.z..=max.z {
            for y in min.y..=max.y {
                for x in min.x..=max.x {
                    self.set(x as u32, y as u32, z as u32, material_id);
                }
            }
        }
    }

    /// Paints materials based on height (Y coordinate).
    ///
    /// Useful for basic terrain layering (e.g., grass on top, dirt below, stone at bottom).
    ///
    /// # Arguments
    /// * `layers` - Slice of (max_height, material_id) pairs, processed bottom to top
    ///
    /// # Example
    ///
    /// ```
    /// use bevy_painter::material_field::MaterialField;
    ///
    /// let mut field = MaterialField::new();
    /// field.paint_height_layers(&[
    ///     (8, 0),   // Stone below y=8
    ///     (20, 1),  // Dirt from y=8 to y=20
    ///     (32, 2),  // Grass above y=20
    /// ]);
    /// ```
    pub fn paint_height_layers(&mut self, layers: &[(u32, u8)]) {
        for z in 0..FIELD_SIZE.z {
            for y in 0..FIELD_SIZE.y {
                for x in 0..FIELD_SIZE.x {
                    let material = layers
                        .iter()
                        .find(|(max_y, _)| y < *max_y)
                        .map(|(_, mat)| *mat)
                        .unwrap_or(0);
                    self.set(x, y, z, material);
                }
            }
        }
    }

    /// Paints materials based on a 3D noise function.
    ///
    /// # Arguments
    /// * `sampler` - Function that takes grid coordinates and returns a material ID
    pub fn paint_with<F>(&mut self, sampler: F)
    where
        F: Fn(UVec3) -> u8,
    {
        for z in 0..FIELD_SIZE.z {
            for y in 0..FIELD_SIZE.y {
                for x in 0..FIELD_SIZE.x {
                    let material = sampler(uvec3(x, y, z));
                    self.set(x, y, z, material);
                }
            }
        }
    }
}

/// Marker component indicating this chunk's material field needs processing.
///
/// Added automatically when `MaterialField` changes. The plugin removes it
/// after injecting material attributes into the mesh.
#[derive(Component, Clone, Copy, Default, Debug)]
pub struct MaterialFieldDirty;

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_paint_sphere() {
        let mut field = MaterialField::new();
        field.paint_sphere(ivec3(16, 16, 16), 3, 7);

        // Center should be painted
        assert_eq!(field.get(16, 16, 16), 7);
        // Just outside should not be painted
        assert_eq!(field.get(16, 16, 20), 0);
    }
}
