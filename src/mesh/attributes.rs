//! Custom vertex attributes for material blending and visibility.

use bevy::mesh::MeshVertexAttribute;
use bevy::render::render_resource::VertexFormat;

/// Vertex attribute containing up to 4 material IDs packed as `[u8; 4]` into a `u32`.
pub const ATTRIBUTE_MATERIAL_IDS: MeshVertexAttribute =
    MeshVertexAttribute::new("MaterialIds", 988540920, VertexFormat::Uint32);

/// Vertex attribute containing blend weights for up to 4 materials packed as `[u8; 4]` into a `u32`.
pub const ATTRIBUTE_MATERIAL_WEIGHTS: MeshVertexAttribute =
    MeshVertexAttribute::new("MaterialWeights", 988540921, VertexFormat::Uint32);

/// Vertex attribute for visibility masking.
///
/// A single f32 from 0.0 (hidden/discard) to 1.0 (fully visible).
/// The shader will discard fragments below a configurable threshold.
pub const ATTRIBUTE_VISIBILITY: MeshVertexAttribute =
    MeshVertexAttribute::new("Visibility", 988540922, VertexFormat::Float32);
