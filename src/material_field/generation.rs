//! Layer-based material field generation system.

use super::MaterialField;
use bevy::prelude::*;
use bevy_sculpter::prelude::Field;

// ============================================================================
// Core Traits
// ============================================================================

/// Trait for material generation layers.
///
/// Layers are applied sequentially to build up complex material distributions.
pub trait MaterialLayer: Send + Sync {
    /// Apply this layer's modifications to the material field.
    /// Returns true if any modifications were made.
    fn apply(&self, materials: &mut MaterialField, chunk_pos: IVec3) -> bool;

    /// Optional: return bounds to optimize which chunks this layer affects.
    fn affected_chunks(&self) -> Option<ChunkBounds> {
        None
    }

    /// Optional: layer priority (higher = applied later, can override earlier layers).
    fn priority(&self) -> i32 {
        0
    }
}

/// Spatial bounds for chunk culling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkBounds {
    pub min: IVec3,
    pub max: IVec3,
}

impl ChunkBounds {
    pub fn new(min: IVec3, max: IVec3) -> Self {
        Self { min, max }
    }

    pub fn contains(&self, chunk_pos: IVec3) -> bool {
        chunk_pos.cmpge(self.min).all() && chunk_pos.cmple(self.max).all()
    }

    pub fn from_world_aabb(min_world: Vec3, max_world: Vec3, chunk_size: Vec3) -> Self {
        Self {
            min: (min_world / chunk_size).floor().as_ivec3(),
            max: (max_world / chunk_size).ceil().as_ivec3(),
        }
    }
}

// ============================================================================
// Layer Collection
// ============================================================================

/// A collection of material layers that can be applied to generate material fields.
///
/// # Example
/// ```ignore
/// let generator = MaterialLayerStack::new()
///     .add_layer(FillLayer::new(0))
///     .add_layer(HeightLayer::new(vec![
///         (20.0, 1),  // Stone above 20
///         (0.0, 2),   // Grass at ground level
///     ]));
///
/// let mut field = MaterialField::new();
/// generator.generate(&mut field, chunk_pos, chunk_size);
/// ```
pub struct MaterialLayerStack {
    layers: Vec<Box<dyn MaterialLayer>>,
    chunk_size: Vec3,
}

impl MaterialLayerStack {
    pub fn new(chunk_size: Vec3) -> Self {
        Self {
            layers: Vec::new(),
            chunk_size,
        }
    }

    pub fn add_layer(mut self, layer: impl MaterialLayer + 'static) -> Self {
        self.layers.push(Box::new(layer));
        self
    }

    pub fn add_boxed(mut self, layer: Box<dyn MaterialLayer>) -> Self {
        self.layers.push(layer);
        self
    }

    /// Generate material field for a chunk.
    pub fn generate(&self, materials: &mut MaterialField, chunk_pos: IVec3) {
        // Sort by priority (stable sort preserves insertion order for equal priorities)
        let mut sorted: Vec<_> = self.layers.iter().collect();
        sorted.sort_by_key(|l| l.priority());

        for layer in sorted {
            if let Some(bounds) = layer.affected_chunks() {
                if !bounds.contains(chunk_pos) {
                    continue;
                }
            }
            layer.apply(materials, chunk_pos);
        }
    }

    pub fn chunk_size(&self) -> Vec3 {
        self.chunk_size
    }
}

// ============================================================================
// Common Layers
// ============================================================================

/// Fill entire field with a single material.
#[derive(Clone, Debug)]
pub struct FillLayer {
    pub material_id: u8,
}

impl FillLayer {
    pub fn new(material_id: u8) -> Self {
        Self { material_id }
    }
}

impl MaterialLayer for FillLayer {
    fn apply(&self, materials: &mut MaterialField, _chunk_pos: IVec3) -> bool {
        materials.fill(self.material_id);
        true
    }
}

/// Assign materials based on world height.
///
/// Layers are checked from highest to lowest. First matching height wins.
#[derive(Clone, Debug)]
pub struct HeightLayer {
    /// (max_height, material_id) pairs, applied top to bottom
    pub layers: Vec<(f32, u8)>,
    pub chunk_size: Vec3,
    pub field_size: UVec3,
}

impl HeightLayer {
    pub fn new(layers: Vec<(f32, u8)>, chunk_size: Vec3, field_size: UVec3) -> Self {
        Self {
            layers,
            chunk_size,
            field_size,
        }
    }

    fn world_pos(&self, chunk_pos: IVec3, local_pos: UVec3) -> Vec3 {
        let resolution = self.chunk_size / self.field_size.as_vec3();
        chunk_pos.as_vec3() * self.chunk_size + local_pos.as_vec3() * resolution
    }
}

impl MaterialLayer for HeightLayer {
    fn apply(&self, materials: &mut MaterialField, chunk_pos: IVec3) -> bool {
        let mut modified = false;

        for local_pos in MaterialField::positions() {
            let world_pos = self.world_pos(chunk_pos, local_pos);

            for (max_height, material_id) in &self.layers {
                if world_pos.y <= *max_height {
                    materials.set_uvec3(local_pos, *material_id);
                    modified = true;
                    break;
                }
            }
        }

        modified
    }
}

/// Paint a spherical region with a material.
#[derive(Clone, Debug)]
pub struct SphereLayer {
    pub center: Vec3,
    pub radius: f32,
    pub material_id: u8,
    pub chunk_size: Vec3,
    pub field_size: UVec3,
}

impl SphereLayer {
    pub fn new(
        center: Vec3,
        radius: f32,
        material_id: u8,
        chunk_size: Vec3,
        field_size: UVec3,
    ) -> Self {
        Self {
            center,
            radius,
            material_id,
            chunk_size,
            field_size,
        }
    }

    fn world_pos(&self, chunk_pos: IVec3, local_pos: UVec3) -> Vec3 {
        let resolution = self.chunk_size / self.field_size.as_vec3();
        chunk_pos.as_vec3() * self.chunk_size + local_pos.as_vec3() * resolution
    }
}

impl MaterialLayer for SphereLayer {
    fn apply(&self, materials: &mut MaterialField, chunk_pos: IVec3) -> bool {
        let mut modified = false;
        let radius_sq = self.radius * self.radius;

        for local_pos in MaterialField::positions() {
            let world_pos = self.world_pos(chunk_pos, local_pos);

            if world_pos.distance_squared(self.center) <= radius_sq {
                materials.set_uvec3(local_pos, self.material_id);
                modified = true;
            }
        }

        modified
    }

    fn affected_chunks(&self) -> Option<ChunkBounds> {
        let min_world = self.center - Vec3::splat(self.radius);
        let max_world = self.center + Vec3::splat(self.radius);
        Some(ChunkBounds::from_world_aabb(
            min_world,
            max_world,
            self.chunk_size,
        ))
    }
}

/// Paint a box region with a material.
#[derive(Clone, Debug)]
pub struct BoxLayer {
    pub min: Vec3,
    pub max: Vec3,
    pub material_id: u8,
    pub chunk_size: Vec3,
    pub field_size: UVec3,
}

impl BoxLayer {
    pub fn new(min: Vec3, max: Vec3, material_id: u8, chunk_size: Vec3, field_size: UVec3) -> Self {
        Self {
            min,
            max,
            material_id,
            chunk_size,
            field_size,
        }
    }

    fn world_pos(&self, chunk_pos: IVec3, local_pos: UVec3) -> Vec3 {
        let resolution = self.chunk_size / self.field_size.as_vec3();
        chunk_pos.as_vec3() * self.chunk_size + local_pos.as_vec3() * resolution
    }
}

impl MaterialLayer for BoxLayer {
    fn apply(&self, materials: &mut MaterialField, chunk_pos: IVec3) -> bool {
        let mut modified = false;

        for local_pos in MaterialField::positions() {
            let world_pos = self.world_pos(chunk_pos, local_pos);

            if world_pos.cmpge(self.min).all() && world_pos.cmple(self.max).all() {
                materials.set_uvec3(local_pos, self.material_id);
                modified = true;
            }
        }

        modified
    }

    fn affected_chunks(&self) -> Option<ChunkBounds> {
        Some(ChunkBounds::from_world_aabb(
            self.min,
            self.max,
            self.chunk_size,
        ))
    }
}

/// Apply materials using a sampling function.
///
/// This allows for procedural/noise-based material placement.
#[derive(Clone)]
pub struct SamplerLayer<F>
where
    F: Fn(Vec3) -> u8 + Send + Sync,
{
    pub sampler: F,
    pub chunk_size: Vec3,
    pub field_size: UVec3,
}

impl<F> SamplerLayer<F>
where
    F: Fn(Vec3) -> u8 + Send + Sync,
{
    pub fn new(sampler: F, chunk_size: Vec3, field_size: UVec3) -> Self {
        Self {
            sampler,
            chunk_size,
            field_size,
        }
    }

    fn world_pos(&self, chunk_pos: IVec3, local_pos: UVec3) -> Vec3 {
        let resolution = self.chunk_size / self.field_size.as_vec3();
        chunk_pos.as_vec3() * self.chunk_size + local_pos.as_vec3() * resolution
    }
}

impl<F> MaterialLayer for SamplerLayer<F>
where
    F: Fn(Vec3) -> u8 + Send + Sync,
{
    fn apply(&self, materials: &mut MaterialField, chunk_pos: IVec3) -> bool {
        for local_pos in MaterialField::positions() {
            let world_pos = self.world_pos(chunk_pos, local_pos);
            let material_id = (self.sampler)(world_pos);
            materials.set_uvec3(local_pos, material_id);
        }
        true
    }
}
