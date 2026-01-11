//! Visibility field storage for per-voxel visibility masking.

use bevy::prelude::*;
use bevy_sculpter::field::Field;
use bevy_sculpter::neighbor::{NeighborFace, NeighborFields, NeighborSlice};

use super::{FIELD_SIZE, FIELD_VOLUME};

/// A 3D grid of visibility flags for voxel terrain.
///
/// Each voxel stores a `u8` where 0 = hidden, 255 = fully visible.
/// Values in between allow for soft transitions at boundaries.
#[derive(Component, Clone, Debug)]
pub struct VisibilityField(pub Vec<u8>);

impl Default for VisibilityField {
    fn default() -> Self {
        // Default to fully visible
        Self(vec![255; FIELD_VOLUME])
    }
}

impl Field<u8> for VisibilityField {
    const SIZE: UVec3 = FIELD_SIZE;
    const DEFAULT: u8 = 0; // Out of bounds = hidden

    #[inline]
    fn data(&self) -> &[u8] {
        &self.0
    }

    #[inline]
    fn data_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

impl VisibilityField {
    /// Creates a new visibility field with all voxels hidden.
    pub fn new_hidden() -> Self {
        Self(vec![0; FIELD_VOLUME])
    }

    /// Creates a new visibility field with all voxels visible.
    pub fn new_visible() -> Self {
        Self::default()
    }

    /// Sets visibility at a position (true = visible, false = hidden).
    pub fn set_visible(&mut self, x: u32, y: u32, z: u32, visible: bool) {
        self.set(x, y, z, if visible { 255 } else { 0 });
    }

    /// Gets visibility at a position as a bool.
    pub fn is_visible(&self, x: u32, y: u32, z: u32) -> bool {
        self.get(x, y, z) > 0
    }

    /// Reveals a spherical region.
    pub fn reveal_sphere(&mut self, center: Vec3, radius: f32) {
        use bevy_sculpter::field::FieldSphereOps;
        self.fill_sphere(center, radius, 255);
    }

    /// Hides a spherical region.
    pub fn hide_sphere(&mut self, center: Vec3, radius: f32) {
        use bevy_sculpter::field::FieldSphereOps;
        self.fill_sphere(center, radius, 0);
    }

    /// Reveals a box region.
    pub fn reveal_box(&mut self, min: IVec3, max: IVec3) {
        use bevy_sculpter::field::FieldBoxOps;
        self.fill_box(min, max, 255);
    }

    /// Hides a box region.
    pub fn hide_box(&mut self, min: IVec3, max: IVec3) {
        use bevy_sculpter::field::FieldBoxOps;
        self.fill_box(min, max, 0);
    }
}

/// Neighbor slice for visibility field data.
pub type VisibilitySlice = NeighborSlice<u8>;

/// Cached neighbor visibility data for seamless meshing.
pub type NeighborVisibilityFields = NeighborFields<u8>;

/// Extension trait for creating visibility slices.
pub trait VisibilitySliceExt {
    fn from_visibility_field(field: &VisibilityField, face: NeighborFace) -> Self;
}

impl VisibilitySliceExt for VisibilitySlice {
    fn from_visibility_field(field: &VisibilityField, face: NeighborFace) -> Self {
        Self::from_field(field, face)
    }
}

/// Marker component indicating visibility field changed.
#[derive(Component, Clone, Copy, Default, Debug)]
pub struct VisibilityFieldDirty;
