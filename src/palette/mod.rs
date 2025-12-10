//! Texture palette utilities for triplanar voxel materials.
//!
//! Provides a builder API for constructing material extensions with
//! texture arrays and per-material properties.

mod builder;
mod properties;

pub use builder::PaletteBuilder;
pub use properties::{MAX_MATERIALS, MaterialPropertiesGpu, PaletteMaterial};
