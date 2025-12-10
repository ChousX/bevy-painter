//! Neighbor material field data for seamless chunk boundaries.

use bevy::prelude::*;

use super::field::{MaterialField, FIELD_SIZE};

/// How many planes of neighbor data to store.
pub const NEIGHBOR_DEPTH: u32 = 2;

/// Cached neighbor material data for seamless meshing.
///
/// Mirrors `bevy_sculpter::neighbor::NeighborDensityFields` but for materials.
#[derive(Component, Clone, Debug, Default)]
pub struct NeighborMaterialFields {
    /// Neighbor slices indexed by face (0=NegX, 1=PosX, 2=NegY, etc.)
    pub neighbors: [Option<NeighborMaterialSlice>; 6],
}

/// Stores boundary planes of material data from a neighboring chunk.
#[derive(Clone, Debug)]
pub struct NeighborMaterialSlice {
    /// Flattened data: `[depth][b][a]`
    pub data: Vec<u8>,
    /// Size along first axis.
    pub size_a: u32,
    /// Size along second axis.
    pub size_b: u32,
    /// Number of planes stored.
    pub depth: u32,
}

impl NeighborMaterialSlice {
    /// Creates a slice from a neighbor chunk's boundary planes.
    pub fn from_field(field: &MaterialField, face: NeighborFace) -> Self {
        let (size_a, size_b, sampler): (u32, u32, Box<dyn Fn(u32, u32, u32) -> u8>) = match face {
            NeighborFace::NegX => (
                FIELD_SIZE.y,
                FIELD_SIZE.z,
                Box::new(|a, b, depth| {
                    let x = FIELD_SIZE.x.saturating_sub(1 + depth);
                    field.get(x, a, b)
                }),
            ),
            NeighborFace::PosX => (
                FIELD_SIZE.y,
                FIELD_SIZE.z,
                Box::new(|a, b, depth| field.get(depth.min(FIELD_SIZE.x - 1), a, b)),
            ),
            NeighborFace::NegY => (
                FIELD_SIZE.x,
                FIELD_SIZE.z,
                Box::new(|a, b, depth| {
                    let y = FIELD_SIZE.y.saturating_sub(1 + depth);
                    field.get(a, y, b)
                }),
            ),
            NeighborFace::PosY => (
                FIELD_SIZE.x,
                FIELD_SIZE.z,
                Box::new(|a, b, depth| field.get(a, depth.min(FIELD_SIZE.y - 1), b)),
            ),
            NeighborFace::NegZ => (
                FIELD_SIZE.x,
                FIELD_SIZE.y,
                Box::new(|a, b, depth| {
                    let z = FIELD_SIZE.z.saturating_sub(1 + depth);
                    field.get(a, b, z)
                }),
            ),
            NeighborFace::PosZ => (
                FIELD_SIZE.x,
                FIELD_SIZE.y,
                Box::new(|a, b, depth| field.get(a, b, depth.min(FIELD_SIZE.z - 1))),
            ),
        };

        let mut data = Vec::with_capacity((size_a * size_b * NEIGHBOR_DEPTH) as usize);
        for depth in 0..NEIGHBOR_DEPTH {
            for b in 0..size_b {
                for a in 0..size_a {
                    data.push(sampler(a, b, depth));
                }
            }
        }

        Self {
            data,
            size_a,
            size_b,
            depth: NEIGHBOR_DEPTH,
        }
    }

    /// Gets the material at (a, b) with depth offset.
    #[inline]
    pub fn get(&self, a: u32, b: u32, depth: u32) -> u8 {
        if a < self.size_a && b < self.size_b && depth < self.depth {
            let idx = (a + b * self.size_a + depth * self.size_a * self.size_b) as usize;
            self.data[idx]
        } else {
            0
        }
    }
}

/// Identifies a face of a chunk for neighbor lookups.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NeighborFace {
    NegX = 0,
    PosX = 1,
    NegY = 2,
    PosY = 3,
    NegZ = 4,
    PosZ = 5,
}

impl NeighborFace {
    pub const ALL: [Self; 6] = [
        Self::NegX,
        Self::PosX,
        Self::NegY,
        Self::PosY,
        Self::NegZ,
        Self::PosZ,
    ];

    pub fn offset(&self) -> IVec3 {
        match self {
            Self::NegX => ivec3(-1, 0, 0),
            Self::PosX => ivec3(1, 0, 0),
            Self::NegY => ivec3(0, -1, 0),
            Self::PosY => ivec3(0, 1, 0),
            Self::NegZ => ivec3(0, 0, -1),
            Self::PosZ => ivec3(0, 0, 1),
        }
    }
}
