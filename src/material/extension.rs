//! Material extension for triplanar voxel rendering.

use bevy::pbr::{ExtendedMaterial, MaterialExtension, StandardMaterial};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};

use crate::palette::TexturePalette;

/// Shader asset paths.
const TRIPLANAR_SHADER_PATH: &str = "shaders/triplanar_extension.wgsl";

/// Convenience type alias for the complete triplanar voxel material.
///
/// This combines Bevy's [`StandardMaterial`] with [`TriplanarExtension`],
/// inheriting all PBR properties while adding triplanar mapping and
/// multi-material blending.
///
/// # Example
///
/// ```ignore
/// use bevy_painter::prelude::*;
/// use bevy::pbr::StandardMaterial;
///
/// let material = TriplanarVoxelMaterial {
///     base: StandardMaterial {
///         perceptual_roughness: 0.8,
///         ..default()
///     },
///     extension: TriplanarExtension {
///         palette: palette_handle,
///         texture_scale: 1.0,
///         blend_sharpness: 4.0,
///         ..default()
///     },
/// };
/// ```
pub type TriplanarVoxelMaterial = ExtendedMaterial<StandardMaterial, TriplanarExtension>;

/// Material extension that adds triplanar mapping and multi-material blending.
///
/// This extension works with [`StandardMaterial`] to provide:
/// - Triplanar texture projection (no UV coordinates needed)
/// - Blending of up to 4 materials per vertex
/// - Per-material texture scaling and blend sharpness
/// - Optional normal mapping with proper triplanar blending
///
/// # Vertex Requirements
///
/// Meshes using this material must have the following vertex attributes:
/// - `ATTRIBUTE_POSITION` - Vertex position
/// - `ATTRIBUTE_NORMAL` - Vertex normal
/// - `ATTRIBUTE_MATERIAL_IDS` - Packed material indices `[u8; 4]` as `u32`
/// - `ATTRIBUTE_MATERIAL_WEIGHTS` - Packed blend weights `[u8; 4]` as `u32`
///
/// Use [`TriplanarMeshBuilder`](crate::mesh::TriplanarMeshBuilder) to create
/// compatible meshes.
#[derive(Asset, AsBindGroup, TypePath, Clone, Debug)]
pub struct TriplanarExtension {
    /// The texture palette containing all material textures.
    ///
    /// This palette defines which textures are available and their properties.
    #[dependency]
    pub palette: Handle<TexturePalette>,

    /// Global texture scale multiplier.
    ///
    /// This is multiplied with each material's individual `texture_scale`.
    /// A value of 1.0 uses the material's scale directly.
    ///
    /// Default: 1.0
    #[uniform(100)]
    pub settings: TriplanarSettings,

    /// Use biplanar mapping for color textures (faster, slightly lower quality).
    ///
    /// When `true`, albedo and ARM textures use biplanar mapping (2 samples)
    /// instead of full triplanar (3 samples). Normal maps always use full
    /// triplanar for quality.
    ///
    /// Default: `true`
    pub use_biplanar_color: bool,

    /// Enable triplanar normal mapping.
    ///
    /// Requires the palette to have a normal texture array.
    /// When `false`, normals come from mesh vertex normals only.
    ///
    /// Default: `true`
    pub enable_normal_maps: bool,
}

/// GPU-side settings for triplanar rendering.
#[derive(Clone, Copy, Debug, Default, ShaderType)]
pub struct TriplanarSettings {
    /// Global texture scale multiplier.
    pub texture_scale: f32,

    /// Global blend sharpness multiplier.
    pub blend_sharpness: f32,

    /// Flags for shader features.
    /// Bit 0: use_biplanar_color
    /// Bit 1: enable_normal_maps
    /// Bit 2: has_arm_texture
    pub flags: u32,

    /// Padding for alignment.
    pub _padding: f32,
}

impl TriplanarSettings {
    /// Flag: Use biplanar mapping for color textures.
    pub const FLAG_USE_BIPLANAR: u32 = 1 << 0;
    /// Flag: Enable normal mapping.
    pub const FLAG_ENABLE_NORMALS: u32 = 1 << 1;
    /// Flag: Palette has ARM texture.
    pub const FLAG_HAS_ARM: u32 = 1 << 2;
}

impl Default for TriplanarExtension {
    fn default() -> Self {
        Self {
            palette: Handle::default(),
            settings: TriplanarSettings {
                texture_scale: 1.0,
                blend_sharpness: 4.0,
                flags: TriplanarSettings::FLAG_USE_BIPLANAR
                    | TriplanarSettings::FLAG_ENABLE_NORMALS,
                _padding: 0.0,
            },
            use_biplanar_color: true,
            enable_normal_maps: true,
        }
    }
}

impl TriplanarExtension {
    /// Create a new triplanar extension with a palette.
    pub fn new(palette: Handle<TexturePalette>) -> Self {
        Self {
            palette,
            ..default()
        }
    }

    /// Set the global texture scale.
    pub fn with_texture_scale(mut self, scale: f32) -> Self {
        self.settings.texture_scale = scale;
        self
    }

    /// Set the global blend sharpness.
    pub fn with_blend_sharpness(mut self, sharpness: f32) -> Self {
        self.settings.blend_sharpness = sharpness;
        self
    }

    /// Enable or disable biplanar color mapping.
    pub fn with_biplanar_color(mut self, enable: bool) -> Self {
        self.use_biplanar_color = enable;
        self.update_flags();
        self
    }

    /// Enable or disable normal mapping.
    pub fn with_normal_maps(mut self, enable: bool) -> Self {
        self.enable_normal_maps = enable;
        self.update_flags();
        self
    }

    /// Update the flags based on current settings.
    fn update_flags(&mut self) {
        let mut flags = 0u32;
        if self.use_biplanar_color {
            flags |= TriplanarSettings::FLAG_USE_BIPLANAR;
        }
        if self.enable_normal_maps {
            flags |= TriplanarSettings::FLAG_ENABLE_NORMALS;
        }
        self.settings.flags = flags;
    }
}

impl MaterialExtension for TriplanarExtension {
    fn vertex_shader() -> ShaderRef {
        TRIPLANAR_SHADER_PATH.into()
    }

    fn fragment_shader() -> ShaderRef {
        TRIPLANAR_SHADER_PATH.into()
    }

    fn deferred_vertex_shader() -> ShaderRef {
        TRIPLANAR_SHADER_PATH.into()
    }

    fn deferred_fragment_shader() -> ShaderRef {
        TRIPLANAR_SHADER_PATH.into()
    }
}

/// Key for specializing the triplanar material pipeline.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct TriplanarMaterialKey {
    /// Whether the palette has normal maps.
    pub has_normal_maps: bool,
    /// Whether the palette has ARM textures.
    pub has_arm: bool,
    /// Whether to use biplanar mapping for colors.
    pub use_biplanar: bool,
}

impl From<&TriplanarExtension> for TriplanarMaterialKey {
    fn from(ext: &TriplanarExtension) -> Self {
        Self {
            has_normal_maps: ext.enable_normal_maps,
            has_arm: false, // Will be updated based on palette
            use_biplanar: ext.use_biplanar_color,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_extension() {
        let ext = TriplanarExtension::default();

        assert_eq!(ext.settings.texture_scale, 1.0);
        assert_eq!(ext.settings.blend_sharpness, 4.0);
        assert!(ext.use_biplanar_color);
        assert!(ext.enable_normal_maps);
    }

    #[test]
    fn test_extension_builder() {
        let ext = TriplanarExtension::new(Handle::default())
            .with_texture_scale(2.0)
            .with_blend_sharpness(8.0)
            .with_biplanar_color(false)
            .with_normal_maps(false);

        assert_eq!(ext.settings.texture_scale, 2.0);
        assert_eq!(ext.settings.blend_sharpness, 8.0);
        assert!(!ext.use_biplanar_color);
        assert!(!ext.enable_normal_maps);
    }

    #[test]
    fn test_flags() {
        let mut ext = TriplanarExtension::default();

        // Default: both enabled
        assert!(ext.settings.flags & TriplanarSettings::FLAG_USE_BIPLANAR != 0);
        assert!(ext.settings.flags & TriplanarSettings::FLAG_ENABLE_NORMALS != 0);

        // Disable biplanar
        ext = ext.with_biplanar_color(false);
        assert!(ext.settings.flags & TriplanarSettings::FLAG_USE_BIPLANAR == 0);
        assert!(ext.settings.flags & TriplanarSettings::FLAG_ENABLE_NORMALS != 0);

        // Disable normals
        ext = ext.with_normal_maps(false);
        assert!(ext.settings.flags & TriplanarSettings::FLAG_USE_BIPLANAR == 0);
        assert!(ext.settings.flags & TriplanarSettings::FLAG_ENABLE_NORMALS == 0);
    }
}
