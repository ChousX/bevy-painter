//! Core material field storage.
use bevy::prelude::*;

use crate::{DENSITY_FIELD_SIZE, FIELD_VOLUME};

/// Default material ID for uninitialized voxels.
pub const DEFAULT_MATERIAL: u8 = 0;

/// Per-voxel material storage for a chunk.
///
/// Stores a single `u8` material ID per voxel, parallel to [`DensityField`].
/// Material blending is computed during mesh generation based on neighboring
/// materials and density values.
///
/// # Memory
///
/// 32×32×32 = 32,768 bytes (32KB) per chunk.
///
/// # Coordinate System
///
/// Uses the same coordinate system as [`DensityField`]: `(x, y, z)` where
/// each axis ranges from `0` to `DENSITY_FIELD_SIZE - 1` (typically 0-31).
#[derive(Component, Clone)]
pub struct MaterialField {
    data: Box<[u8; FIELD_VOLUME]>,
}

impl Default for MaterialField {
    fn default() -> Self {
        Self::new()
    }
}

impl MaterialField {
    /// Creates a new material field filled with [`DEFAULT_MATERIAL`].
    pub fn new() -> Self {
        Self::filled(DEFAULT_MATERIAL)
    }

    /// Creates a material field filled with a single material ID.
    pub fn filled(material_id: u8) -> Self {
        Self {
            data: Box::new([material_id; FIELD_VOLUME]),
        }
    }

    /// Creates a material field by evaluating a function at each voxel.
    ///
    /// # Arguments
    /// * `f` - Function taking grid coordinates `(x, y, z)` and returning material ID
    ///
    /// # Example
    /// ```
    /// use bevy_sculpter::material_field::MaterialField;
    ///
    /// // Layered materials by height
    /// let field = MaterialField::from_fn(|x, y, z| {
    ///     if y > 24 { 2 }
    ///     else if y > 16 { 1 }
    ///     else { 0 }
    /// });
    /// ```
    pub fn from_fn<F>(mut f: F) -> Self
    where
        F: FnMut(u32, u32, u32) -> u8,
    {
        let mut data = Box::new([0u8; FIELD_VOLUME]);
        for z in 0..DENSITY_FIELD_SIZE.z {
            for y in 0..DENSITY_FIELD_SIZE.y {
                for x in 0..DENSITY_FIELD_SIZE.x {
                    let idx = Self::index(x, y, z);
                    data[idx] = f(x, y, z);
                }
            }
        }
        Self { data }
    }

    /// Gets the material ID at the given coordinates.
    ///
    /// # Panics
    /// Panics if coordinates are out of bounds.
    #[inline]
    pub fn get(&self, x: u32, y: u32, z: u32) -> u8 {
        self.data[Self::index(x, y, z)]
    }

    /// Sets the material ID at the given coordinates.
    ///
    /// # Panics
    /// Panics if coordinates are out of bounds.
    #[inline]
    pub fn set(&mut self, x: u32, y: u32, z: u32, material_id: u8) {
        self.data[Self::index(x, y, z)] = material_id;
    }

    /// Gets the material ID at signed coordinates, returning `None` if out of bounds.
    #[inline]
    pub fn get_signed(&self, x: i32, y: i32, z: i32) -> Option<u8> {
        if x < 0 || y < 0 || z < 0 {
            return None;
        }
        let (x, y, z) = (x as u32, y as u32, z as u32);
        if x >= DENSITY_FIELD_SIZE.x || y >= DENSITY_FIELD_SIZE.y || z >= DENSITY_FIELD_SIZE.z {
            return None;
        }
        Some(self.get(x, y, z))
    }

    /// Sets the material ID at signed coordinates. No-op if out of bounds.
    #[inline]
    pub fn set_signed(&mut self, x: i32, y: i32, z: i32, material_id: u8) {
        if x < 0 || y < 0 || z < 0 {
            return;
        }
        let (x, y, z) = (x as u32, y as u32, z as u32);
        if x >= DENSITY_FIELD_SIZE.x || y >= DENSITY_FIELD_SIZE.y || z >= DENSITY_FIELD_SIZE.z {
            return;
        }
        self.set(x, y, z, material_id);
    }

    /// Fills the entire field with a single material ID.
    pub fn fill(&mut self, material_id: u8) {
        self.data.fill(material_id);
    }

    /// Fills the field using a height-based function.
    ///
    /// # Arguments
    /// * `f` - Function taking Y coordinate (0-31) and returning material ID
    ///
    /// # Example
    /// ```
    /// use bevy_sculpter::material_field::MaterialField;
    ///
    /// let mut field = MaterialField::new();
    /// field.fill_by_height(|y| if y > 16.0 { 1 } else { 0 });
    /// ```
    pub fn fill_by_height<F>(&mut self, mut f: F)
    where
        F: FnMut(f32) -> u8,
    {
        for z in 0..DENSITY_FIELD_SIZE.z {
            for y in 0..DENSITY_FIELD_SIZE.y {
                let mat = f(y as f32);
                for x in 0..DENSITY_FIELD_SIZE.x {
                    self.set(x, y, z, mat);
                }
            }
        }
    }

    /// Fills the field using world-space height, accounting for chunk position.
    ///
    /// # Arguments
    /// * `chunk_pos` - Chunk position in chunk coordinates
    /// * `chunk_size` - World-space size of a chunk
    /// * `f` - Function taking world Y coordinate and returning material ID
    pub fn fill_by_world_height<F>(&mut self, chunk_pos: IVec3, chunk_size: Vec3, mut f: F)
    where
        F: FnMut(f32) -> u8,
    {
        let voxel_size = chunk_size / DENSITY_FIELD_SIZE.as_vec3();
        let base_y = chunk_pos.y as f32 * chunk_size.y;

        for z in 0..DENSITY_FIELD_SIZE.z {
            for y in 0..DENSITY_FIELD_SIZE.y {
                let world_y = base_y + y as f32 * voxel_size.y;
                let mat = f(world_y);
                for x in 0..DENSITY_FIELD_SIZE.x {
                    self.set(x, y, z, mat);
                }
            }
        }
    }

    /// Returns raw access to the underlying data.
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        &self.data[..]
    }

    /// Returns mutable raw access to the underlying data.
    #[inline]
    pub fn as_slice_mut(&mut self) -> &mut [u8] {
        &mut self.data[..]
    }

    /// Computes the linear index for 3D coordinates.
    #[inline]
    fn index(x: u32, y: u32, z: u32) -> usize {
        (z * DENSITY_FIELD_SIZE.y * DENSITY_FIELD_SIZE.x 
            + y * DENSITY_FIELD_SIZE.x 
            + x) as usize
    }
}

impl std::fmt::Debug for MaterialField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Count material distribution
        let mut counts = [0u32; 256];
        for &mat in self.data.iter() {
            counts[mat as usize] += 1;
        }
        
        let non_zero: Vec<_> = counts
            .iter()
            .enumerate()
            .filter(|(_, &c)| c > 0)
            .map(|(id, &count)| (id, count))
            .collect();

        f.debug_struct("MaterialField")
            .field("materials", &non_zero)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_filled_with_default() {
        let field = MaterialField::new();
        assert_eq!(field.get(0, 0, 0), DEFAULT_MATERIAL);
        assert_eq!(field.get(31, 31, 31), DEFAULT_MATERIAL);
    }

    #[test]
    fn test_set_get() {
        let mut field = MaterialField::new();
        field.set(5, 10, 15, 42);
        assert_eq!(field.get(5, 10, 15), 42);
        assert_eq!(field.get(0, 0, 0), DEFAULT_MATERIAL);
    }

    #[test]
    fn test_from_fn() {
        let field = MaterialField::from_fn(|x, y, z| ((x + y + z) % 4) as u8);
        assert_eq!(field.get(0, 0, 0), 0);
        assert_eq!(field.get(1, 0, 0), 1);
        assert_eq!(field.get(1, 1, 1), 3);
    }

    #[test]
    fn test_signed_access() {
        let mut field = MaterialField::new();
        
        assert!(field.get_signed(-1, 0, 0).is_none());
        assert!(field.get_signed(0, 32, 0).is_none());
        assert!(field.get_signed(5, 5, 5).is_some());
        
        field.set_signed(-1, 0, 0, 99); // Should be no-op
        field.set_signed(5, 5, 5, 77);
        assert_eq!(field.get(5, 5, 5), 77);
    }
}
