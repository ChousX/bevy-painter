//! Texture palette for triplanar voxel materials.
//!
//! A [`TexturePalette`] defines a collection of materials that can be
//! blended together on voxel meshes. Each material corresponds to a layer
//! in the texture arrays.

mod asset;
mod builder;
mod properties;
mod validation;

pub use asset::TexturePalette;
pub use builder::PaletteBuilder;
pub use properties::PaletteMaterial;
pub use validation::PaletteValidationError;
