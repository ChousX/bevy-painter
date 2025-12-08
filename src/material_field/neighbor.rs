//! Neighbor material data for seamless chunk boundaries.

use bevy::prelude::*;

use crate::{DENSITY_FIELD_SIZE, neighbor::NeighborFace};
use super::MaterialField;

/// Depth of neighbor data to cache (matching density neighbor depth).
pub const NEIGHBOR_DEPTH: u32 = 2;

/// Cached neighbor material data for seamless meshing.
///
/// Stores boundary slices from up to 6 neighboring chunks, allowing
/// the mesher to sample materials beyond chunk boundaries.
#[derive(Component, Clone, Debug, Default)]
pub struct NeighborMaterialFields {
    /// Neighbor slices indexed by [`NeighborFace`].
    pub neighbors: [Option<MaterialNeighborSlice>; 6],
}

impl NeighborMaterialFields {
    /// Gets the material at a position that may extend beyond chunk boundaries.
    ///
    /// Returns `None` if the position is out of range or neighbor data isn't available.
    pub fn get_extended(&self, field: &MaterialField, x: i32, y: i32, z: i32) -> Option<u8> {
        let size = DENSITY_FIELD_SIZE.as_ivec3();

        // Within bounds - use main field
        if x >= 0 && x < size.x && y >= 0 && y < size.y && z >= 0 && z < size.z {
            return Some(field.get(x as u32, y as u32, z as u32));
        }

        // Check each face for out-of-bounds access
        if x >= size.x {
            let depth = (x - size.x) as u32;
            if depth < NEIGHBOR_DEPTH {
                if let Some(slice) = &self.neighbors[NeighborFace::PosX as usize] {
                    return slice.get(depth, y as u32, z as u32);
                }
            }
        } else if x < 0 {
            let depth = (-1 - x) as u32;
            if depth < NEIGHBOR_DEPTH {
                if let Some(slice) = &self.neighbors[NeighborFace::NegX as usize] {
                    return slice.get(depth, y as u32, z as u32);
                }
            }
        }

        if y >= size.y {
            let depth = (y - size.y) as u32;
            if depth < NEIGHBOR_DEPTH {
                if let Some(slice) = &self.neighbors[NeighborFace::PosY as usize] {
                    return slice.get(depth, x as u32, z as u32);
                }
            }
        } else if y < 0 {
            let depth = (-1 - y) as u32;
            if depth < NEIGHBOR_DEPTH {
                if let Some(slice) = &self.neighbors[NeighborFace::NegY as usize] {
                    return slice.get(depth, x as u32, z as u32);
                }
            }
        }

        if z >= size.z {
            let depth = (z - size.z) as u32;
            if depth < NEIGHBOR_DEPTH {
                if let Some(slice) = &self.neighbors[NeighborFace::PosZ as usize] {
                    return slice.get(depth, x as u32, y as u32);
                }
            }
        } else if z < 0 {
            let depth = (-1 - z) as u32;
            if depth < NEIGHBOR_DEPTH {
                if let Some(slice) = &self.neighbors[NeighborFace::NegZ as usize] {
                    return slice.get(depth, x as u32, y as u32);
                }
            }
        }

        None
    }
}

/// Stores boundary planes of material data from a neighboring chunk.
#[derive(Clone, Debug)]
pub struct MaterialNeighborSlice {
    /// Flattened data: `[depth][b][a]`
    data: Vec<u8>,
    size_a: u32,
    size_b: u32,
    depth: u32,
}

impl MaterialNeighborSlice {
    /// Creates a slice from a neighbor chunk's boundary.
    pub fn from_field(field: &MaterialField, face: NeighborFace) -> Self {
        let (size_a, size_b, depth) = match face {
            NeighborFace::NegX | NeighborFace::PosX => {
                (DENSITY_FIELD_SIZE.y, DENSITY_FIELD_SIZE.z, NEIGHBOR_DEPTH)
            }
            NeighborFace::NegY | NeighborFace::PosY => {
                (DENSITY_FIELD_SIZE.x, DENSITY_FIELD_SIZE.z, NEIGHBOR_DEPTH)
            }
            NeighborFace::NegZ | NeighborFace::PosZ => {
                (DENSITY_FIELD_SIZE.x, DENSITY_FIELD_SIZE.y, NEIGHBOR_DEPTH)
            }
        };

        let mut data = vec![0u8; (size_a * size_b * depth) as usize];

        for d in 0..depth {
            for b in 0..size_b {
                for a in 0..size_a {
                    let (x, y, z) = match face {
                        NeighborFace::NegX => (d, a, b),
                        NeighborFace::PosX => (DENSITY_FIELD_SIZE.x - 1 - d, a, b),
                        NeighborFace::NegY => (a, d, b),
                        NeighborFace::PosY => (a, DENSITY_FIELD_SIZE.y - 1 - d, b),
                        NeighborFace::NegZ => (a, b, d),
                        NeighborFace::PosZ => (a, b, DENSITY_FIELD_SIZE.z - 1 - d),
                    };

                    let idx = (d * size_a * size_b + b * size_a + a) as usize;
                    data[idx] = field.get(x, y, z);
                }
            }
        }

        Self { data, size_a, size_b, depth }
    }

    /// Gets material at local slice coordinates.
    fn get(&self, depth: u32, a: u32, b: u32) -> Option<u8> {
        if depth >= self.depth || a >= self.size_a || b >= self.size_b {
            return None;
        }
        let idx = (depth * self.size_a * self.size_b + b * self.size_a + a) as usize;
        Some(self.data[idx])
    }
}

/// System to gather neighbor material data before meshing.
///
/// Add this to your app if using `MaterialField` with chunked terrain:
///
/// ```ignore
/// app.add_systems(Update, gather_neighbor_materials.before(remesh_system));
/// ```
pub fn gather_neighbor_materials(
    mut commands: Commands,
    chunks: Query<(Entity, &ChunkPos, &MaterialField), With<DensityFieldDirty>>,
    all_chunks: Query<(&ChunkPos, &MaterialField)>,
) {
    use crate::prelude::ChunkPos;
    use crate::density_field::DensityFieldDirty;

    for (entity, pos, _) in chunks.iter() {
        let mut neighbor_fields = NeighborMaterialFields::default();

        for (face_idx, offset) in [
            IVec3::NEG_X,
            IVec3::X,
            IVec3::NEG_Y,
            IVec3::Y,
            IVec3::NEG_Z,
            IVec3::Z,
        ]
        .iter()
        .enumerate()
        {
            let neighbor_pos = pos.0 + *offset;

            for (other_pos, other_field) in all_chunks.iter() {
                if other_pos.0 == neighbor_pos {
                    let face = match face_idx {
                        0 => NeighborFace::NegX,
                        1 => NeighborFace::PosX,
                        2 => NeighborFace::NegY,
                        3 => NeighborFace::PosY,
                        4 => NeighborFace::NegZ,
                        _ => NeighborFace::PosZ,
                    };
                    // For neighbor's PosX face, we need their NegX boundary (opposite)
                    let sample_face = face.opposite();
                    neighbor_fields.neighbors[face_idx] =
                        Some(MaterialNeighborSlice::from_field(other_field, sample_face));
                    break;
                }
            }
        }

        commands.entity(entity).insert(neighbor_fields);
    }
}

/// Extension trait for NeighborFace to get opposite face.
trait NeighborFaceExt {
    fn opposite(&self) -> Self;
}

impl NeighborFaceExt for NeighborFace {
    fn opposite(&self) -> Self {
        match self {
            NeighborFace::NegX => NeighborFace::PosX,
            NeighborFace::PosX => NeighborFace::NegX,
            NeighborFace::NegY => NeighborFace::PosY,
            NeighborFace::PosY => NeighborFace::NegY,
            NeighborFace::NegZ => NeighborFace::PosZ,
            NeighborFace::PosZ => NeighborFace::NegZ,
        }
    }
}
