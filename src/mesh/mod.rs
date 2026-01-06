//! Mesh utilities for triplanar voxel rendering.
//!
//! Material data is packed into vertex colors for compatibility with Bevy's
//! standard vertex pipeline:
//! - color.r: packed material IDs (4x u8 as f32 bitcast)
//! - color.g: packed material weights (4x u8 as f32 bitcast)
//! - color.b: unused (reserved)
//! - color.a: unused (reserved)

use bevy::prelude::*;

mod attributes;
mod builder;
mod vertex_data;

pub use attributes::{ATTRIBUTE_MATERIAL_IDS, ATTRIBUTE_MATERIAL_WEIGHTS};
pub use builder::{MeshTriplanarExt, TriplanarMeshBuilder};
pub use vertex_data::VertexMaterialData;

/// Packs material data into a vertex color value.
/// 
/// Material IDs go into the R channel, weights into G channel.
/// Both are packed as 4x u8 into u32, then bitcast to f32.
pub fn pack_material_to_color(data: &VertexMaterialData) -> [f32; 4] {
    let packed_ids = data.pack_ids();
    let packed_weights = data.pack_weights();
    
    [
        f32::from_bits(packed_ids),
        f32::from_bits(packed_weights),
        0.0,
        1.0, // Alpha = 1 for opaque
    ]
}

/// Unpacks material data from a vertex color value.
pub fn unpack_material_from_color(color: [f32; 4]) -> VertexMaterialData {
    let packed_ids = color[0].to_bits();
    let packed_weights = color[1].to_bits();
    
    VertexMaterialData {
        ids: [
            (packed_ids & 0xFF) as u8,
            ((packed_ids >> 8) & 0xFF) as u8,
            ((packed_ids >> 16) & 0xFF) as u8,
            ((packed_ids >> 24) & 0xFF) as u8,
        ],
        weights: [
            (packed_weights & 0xFF) as u8,
            ((packed_weights >> 8) & 0xFF) as u8,
            ((packed_weights >> 16) & 0xFF) as u8,
            ((packed_weights >> 24) & 0xFF) as u8,
        ],
    }
}

/// Extension trait for adding triplanar material data to existing meshes via vertex colors.
pub trait MeshTriplanarColorExt {
    /// Add material data to mesh via vertex colors.
    ///
    /// The material data slice must have the same length as the vertex count.
    fn with_triplanar_material_colors(self, material_data: &[VertexMaterialData]) -> Self;

    /// Add uniform material to all vertices via vertex colors.
    fn with_uniform_material_color(self, material_id: u8) -> Self;
}

impl MeshTriplanarColorExt for Mesh {
    fn with_triplanar_material_colors(mut self, material_data: &[VertexMaterialData]) -> Self {
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

        let colors: Vec<[f32; 4]> = material_data
            .iter()
            .map(pack_material_to_color)
            .collect();

        self.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
        self
    }

    fn with_uniform_material_color(self, material_id: u8) -> Self {
        let vertex_count = self
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .map(|a| a.len())
            .unwrap_or(0);

        let data = vec![VertexMaterialData::single(material_id); vertex_count];
        self.with_triplanar_material_colors(&data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pack_unpack_roundtrip() {
        let original = VertexMaterialData::blend4(
            [1, 5, 10, 255],
            [0.5, 0.25, 0.15, 0.1],
        );
        
        let packed = pack_material_to_color(&original);
        let unpacked = unpack_material_from_color(packed);
        
        assert_eq!(original.ids, unpacked.ids);
        assert_eq!(original.weights, unpacked.weights);
    }

    #[test]
    fn test_single_material_pack() {
        let data = VertexMaterialData::single(42);
        let packed = pack_material_to_color(&data);
        let unpacked = unpack_material_from_color(packed);
        
        assert_eq!(unpacked.ids[0], 42);
        assert_eq!(unpacked.weights[0], 255);
    }
}
