//! Custom vertex attributes for material blending.

use bevy::mesh::MeshVertexAttribute;
use bevy::render::render_resource::VertexFormat;

/// Vertex attribute containing up to 4 material IDs packed as `[u8; 4]` into a `u32`.
///
/// Each byte represents a material index into the texture palette.
/// Unused slots should be set to 0.
///
/// # Shader Location
/// This attribute is bound to location 2 in the vertex shader.
/// (After position=0, normal=1)
///
/// # Example
/// ```ignore
/// // Vertex using materials 0, 3, and 7
/// let ids: u32 = 0 | (3 << 8) | (7 << 16) | (0 << 24);
/// ```
pub const ATTRIBUTE_MATERIAL_IDS: MeshVertexAttribute =
    MeshVertexAttribute::new("MaterialIds", 988540920, VertexFormat::Uint32);

/// Vertex attribute containing blend weights for up to 4 materials packed as `[u8; 4]` into a `u32`.
///
/// Each byte represents a weight from 0-255. Weights should sum to 255 for correct blending,
/// though the shader will normalize them if they don't.
///
/// # Shader Location
/// This attribute is bound to location 3 in the vertex shader.
/// (After position=0, normal=1, material_ids=2)
///
/// # Example
/// ```ignore
/// // 50% material 0, 50% material 1
/// let weights: u32 = 128 | (127 << 8) | (0 << 16) | (0 << 24);
/// ```
pub const ATTRIBUTE_MATERIAL_WEIGHTS: MeshVertexAttribute =
    MeshVertexAttribute::new("MaterialWeights", 988540921, VertexFormat::Uint32);
