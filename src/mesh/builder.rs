//! Mesh builder for triplanar voxel meshes.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::prelude::*;

use super::{
    attributes::{ATTRIBUTE_MATERIAL_IDS, ATTRIBUTE_MATERIAL_WEIGHTS},
    vertex_data::VertexMaterialData,
};

/// Builder for creating meshes with triplanar material attributes.
///
/// This builder collects vertex data (positions, normals, material data)
/// and produces a Bevy [`Mesh`] with the custom vertex attributes required
/// by [`TriplanarVoxelMaterial`](crate::material::TriplanarVoxelMaterial).
///
/// # Example
/// ```ignore
/// use bevy_triplanar_voxel::mesh::{TriplanarMeshBuilder, VertexMaterialData};
///
/// let mesh = TriplanarMeshBuilder::new()
///     .with_vertex([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], VertexMaterialData::single(0))
///     .with_vertex([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], VertexMaterialData::single(0))
///     .with_vertex([0.5, 0.0, 1.0], [0.0, 1.0, 0.0], VertexMaterialData::blend2_half(0, 1))
///     .with_indices(vec![0, 1, 2])
///     .build();
/// ```
#[derive(Default)]
pub struct TriplanarMeshBuilder {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    material_ids: Vec<u32>,
    material_weights: Vec<u32>,
    indices: Option<Vec<u32>>,
    max_material_id: Option<u8>,
}

impl TriplanarMeshBuilder {
    /// Create a new empty mesh builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a builder with pre-allocated capacity.
    ///
    /// Use this when you know approximately how many vertices you'll have
    /// to avoid reallocations.
    pub fn with_capacity(vertex_count: usize, index_count: usize) -> Self {
        Self {
            positions: Vec::with_capacity(vertex_count),
            normals: Vec::with_capacity(vertex_count),
            material_ids: Vec::with_capacity(vertex_count),
            material_weights: Vec::with_capacity(vertex_count),
            indices: Some(Vec::with_capacity(index_count)),
            max_material_id: None,
        }
    }

    /// Set the maximum valid material ID for validation.
    ///
    /// When set, debug builds will panic if any vertex uses a material ID
    /// greater than this value.
    ///
    /// This is typically set to `palette.materials.len() - 1`.
    pub fn with_max_material_id(mut self, max_id: u8) -> Self {
        self.max_material_id = Some(max_id);
        self
    }

    /// Add a vertex with a single material.
    ///
    /// Convenience method equivalent to:
    /// ```ignore
    /// builder.with_vertex(position, normal, VertexMaterialData::single(material_id))
    /// ```
    pub fn with_vertex_single(
        mut self,
        position: impl Into<[f32; 3]>,
        normal: impl Into<[f32; 3]>,
        material_id: u8,
    ) -> Self {
        self.push_vertex(
            position.into(),
            normal.into(),
            VertexMaterialData::single(material_id),
        );
        self
    }

    /// Add a vertex with material blending data.
    pub fn with_vertex(
        mut self,
        position: impl Into<[f32; 3]>,
        normal: impl Into<[f32; 3]>,
        material_data: VertexMaterialData,
    ) -> Self {
        self.push_vertex(position.into(), normal.into(), material_data);
        self
    }

    /// Add a vertex (mutable version for loops).
    pub fn push_vertex(
        &mut self,
        position: [f32; 3],
        normal: [f32; 3],
        material_data: VertexMaterialData,
    ) {
        #[cfg(debug_assertions)]
        if let Some(max_id) = self.max_material_id {
            for (i, &id) in material_data.ids.iter().enumerate() {
                if material_data.weights[i] > 0 {
                    debug_assert!(
                        id <= max_id,
                        "Material ID {} exceeds maximum {} at vertex {:?}",
                        id,
                        max_id,
                        position
                    );
                }
            }
        }

        self.positions.push(position);
        self.normals.push(normal);
        self.material_ids.push(material_data.pack_ids());
        self.material_weights.push(material_data.pack_weights());
    }

    /// Set the triangle indices.
    pub fn with_indices(mut self, indices: Vec<u32>) -> Self {
        self.indices = Some(indices);
        self
    }

    /// Add indices (mutable version).
    pub fn push_indices(&mut self, indices: &[u32]) {
        self.indices
            .get_or_insert_with(Vec::new)
            .extend_from_slice(indices);
    }

    /// Add a single triangle by vertex indices.
    pub fn push_triangle(&mut self, a: u32, b: u32, c: u32) {
        self.indices
            .get_or_insert_with(Vec::new)
            .extend_from_slice(&[a, b, c]);
    }

    /// Get the current vertex count.
    pub fn vertex_count(&self) -> usize {
        self.positions.len()
    }

    /// Get the current index count.
    pub fn index_count(&self) -> usize {
        self.indices.as_ref().map(|i| i.len()).unwrap_or(0)
    }

    /// Build the final mesh.
    ///
    /// Returns `None` if there are no vertices or indices.
    pub fn build(self) -> Option<Mesh> {
        if self.positions.is_empty() {
            return None;
        }

        let indices = self.indices?;
        if indices.is_empty() {
            return None;
        }

        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
        );

        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals);
        mesh.insert_attribute(ATTRIBUTE_MATERIAL_IDS, self.material_ids);
        mesh.insert_attribute(ATTRIBUTE_MATERIAL_WEIGHTS, self.material_weights);
        mesh.insert_indices(Indices::U32(indices));

        Some(mesh)
    }

    /// Build the mesh, panicking if invalid.
    ///
    /// # Panics
    /// Panics if there are no vertices or indices.
    pub fn build_unwrap(self) -> Mesh {
        self.build().expect("Cannot build empty mesh")
    }
}

/// Extension trait for adding triplanar material data to existing meshes.
pub trait MeshTriplanarExt {
    /// Add material attributes to an existing mesh.
    ///
    /// The material data slice must have the same length as the vertex count.
    ///
    /// # Panics
    /// Panics if `material_data.len()` doesn't match the vertex count.
    fn with_triplanar_materials(self, material_data: &[VertexMaterialData]) -> Self;

    /// Add uniform material to all vertices.
    fn with_uniform_material(self, material_id: u8) -> Self;
}

impl MeshTriplanarExt for Mesh {
    fn with_triplanar_materials(mut self, material_data: &[VertexMaterialData]) -> Self {
        let vertex_count = self
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .map(|a| a.len())
            .unwrap_or(0);

        assert_eq!(
            material_data.len(),
            vertex_count,
            "Material data length ({}) must match vertex count ({})",
            material_data.len(),
            vertex_count
        );

        let ids: Vec<u32> = material_data.iter().map(|d| d.pack_ids()).collect();
        let weights: Vec<u32> = material_data.iter().map(|d| d.pack_weights()).collect();

        self.insert_attribute(ATTRIBUTE_MATERIAL_IDS, ids);
        self.insert_attribute(ATTRIBUTE_MATERIAL_WEIGHTS, weights);

        self
    }

    fn with_uniform_material(self, material_id: u8) -> Self {
        let vertex_count = self
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .map(|a| a.len())
            .unwrap_or(0);

        let data = vec![VertexMaterialData::single(material_id); vertex_count];
        self.with_triplanar_materials(&data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_basic() {
        let mesh = TriplanarMeshBuilder::new()
            .with_vertex_single([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], 0)
            .with_vertex_single([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 0)
            .with_vertex_single([0.5, 0.0, 1.0], [0.0, 1.0, 0.0], 0)
            .with_indices(vec![0, 1, 2])
            .build();

        assert!(mesh.is_some());
        let mesh = mesh.unwrap();

        assert!(mesh.attribute(Mesh::ATTRIBUTE_POSITION).is_some());
        assert!(mesh.attribute(Mesh::ATTRIBUTE_NORMAL).is_some());
        assert!(mesh.attribute(ATTRIBUTE_MATERIAL_IDS).is_some());
        assert!(mesh.attribute(ATTRIBUTE_MATERIAL_WEIGHTS).is_some());
    }

    #[test]
    fn test_builder_empty_returns_none() {
        assert!(TriplanarMeshBuilder::new().build().is_none());

        assert!(
            TriplanarMeshBuilder::new()
                .with_vertex_single([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], 0)
                .build()
                .is_none()
        ); // No indices
    }

    #[test]
    fn test_mesh_extension() {
        let mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );

        // Would need to add positions first in real usage
        // This test just verifies the API compiles
    }
}
