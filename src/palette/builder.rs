//! Builder for constructing triplanar extensions.

use bevy::prelude::*;

use super::properties::{PaletteMaterial, MaterialPropertiesGpu};
use crate::material::TriplanarExtension;

/// Builder for creating [`TriplanarExtension`] instances.
///
/// Provides a fluent API for constructing material extensions with validation.
///
/// # Example
///
/// ```ignore
/// use bevy_painter::palette::{PaletteBuilder, PaletteMaterial};
///
/// let extension = PaletteBuilder::new()
///     .with_albedo(asset_server.load("terrain/albedo.ktx2"))
///     .with_normal(asset_server.load("terrain/normal.ktx2"))
///     .with_arm(asset_server.load("terrain/arm.ktx2"))
///     .add_material(PaletteMaterial::new("grass").with_texture_scale(1.0))
///     .add_material(PaletteMaterial::new("stone").with_texture_scale(0.5))
///     .add_material(PaletteMaterial::new("dirt"))
///     .build();
/// ```
#[derive(Default)]
pub struct PaletteBuilder {
    albedo: Option<Handle<Image>>,
    normal: Option<Handle<Image>>,
    arm: Option<Handle<Image>>,
    materials: Vec<PaletteMaterial>,
    texture_scale: f32,
    blend_sharpness: f32,
    use_biplanar_color: bool,
    enable_normal_maps: bool,
}

impl PaletteBuilder {
    /// Create a new palette builder.
    pub fn new() -> Self {
        Self {
            texture_scale: 1.0,
            blend_sharpness: 4.0,
            use_biplanar_color: true,
            enable_normal_maps: true,
            ..Default::default()
        }
    }

    /// Set the albedo (base color) texture array.
    ///
    /// **Required.** The builder will panic on `build()` if this is not set.
    pub fn with_albedo(mut self, albedo: Handle<Image>) -> Self {
        self.albedo = Some(albedo);
        self
    }

    /// Set the normal map texture array.
    ///
    /// Optional. Enables triplanar normal mapping when provided.
    pub fn with_normal(mut self, normal: Handle<Image>) -> Self {
        self.normal = Some(normal);
        self
    }

    /// Set the ARM (AO/Roughness/Metallic) texture array.
    ///
    /// Optional. Channel layout: R = AO, G = Roughness, B = Metallic.
    pub fn with_arm(mut self, arm: Handle<Image>) -> Self {
        self.arm = Some(arm);
        self
    }

    /// Add a material to the palette.
    ///
    /// Materials are added in order, corresponding to texture array layers.
    pub fn add_material(mut self, material: PaletteMaterial) -> Self {
        self.materials.push(material);
        self
    }

    /// Add multiple materials at once.
    pub fn add_materials(mut self, materials: impl IntoIterator<Item = PaletteMaterial>) -> Self {
        self.materials.extend(materials);
        self
    }

    /// Add a material with just a name, using default properties.
    pub fn add_material_named(self, name: impl Into<String>) -> Self {
        self.add_material(PaletteMaterial::new(name))
    }

    /// Set the global texture scale multiplier.
    pub fn with_texture_scale(mut self, scale: f32) -> Self {
        self.texture_scale = scale;
        self
    }

    /// Set the global blend sharpness multiplier.
    pub fn with_blend_sharpness(mut self, sharpness: f32) -> Self {
        self.blend_sharpness = sharpness;
        self
    }

    /// Enable or disable biplanar color mapping.
    pub fn with_biplanar_color(mut self, enable: bool) -> Self {
        self.use_biplanar_color = enable;
        self
    }

    /// Enable or disable normal mapping.
    pub fn with_normal_maps(mut self, enable: bool) -> Self {
        self.enable_normal_maps = enable;
        self
    }

    /// Build the triplanar extension.
    ///
    /// # Panics
    ///
    /// Panics if no albedo texture was provided.
    pub fn build(self) -> TriplanarExtension {
        let material_properties: Vec<MaterialPropertiesGpu> = self
            .materials
            .iter()
            .map(|m| MaterialPropertiesGpu::from(m))
            .collect();

        TriplanarExtension {
            albedo: self.albedo.expect("Albedo texture is required"),
            normal: self.normal,
            arm: self.arm,
            material_properties,
            texture_scale: self.texture_scale,
            blend_sharpness: self.blend_sharpness,
            use_biplanar_color: self.use_biplanar_color,
            enable_normal_maps: self.enable_normal_maps,
        }
    }

    /// Try to build the triplanar extension.
    ///
    /// Returns `None` if no albedo texture was provided.
    pub fn try_build(self) -> Option<TriplanarExtension> {
        let material_properties: Vec<MaterialPropertiesGpu> = self
            .materials
            .iter()
            .map(|m| MaterialPropertiesGpu::from(m))
            .collect();

        Some(TriplanarExtension {
            albedo: self.albedo?,
            normal: self.normal,
            arm: self.arm,
            material_properties,
            texture_scale: self.texture_scale,
            blend_sharpness: self.blend_sharpness,
            use_biplanar_color: self.use_biplanar_color,
            enable_normal_maps: self.enable_normal_maps,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_basic() {
        let ext = PaletteBuilder::new()
            .with_albedo(Handle::default())
            .add_material_named("grass")
            .add_material_named("stone")
            .build();

        assert_eq!(ext.material_properties.len(), 2);
    }

    #[test]
    fn test_builder_full() {
        let ext = PaletteBuilder::new()
            .with_albedo(Handle::default())
            .with_normal(Handle::default())
            .with_arm(Handle::default())
            .add_material(PaletteMaterial::new("grass").with_texture_scale(2.0))
            .with_texture_scale(1.5)
            .with_blend_sharpness(8.0)
            .build();

        assert!(ext.normal.is_some());
        assert!(ext.arm.is_some());
        assert_eq!(ext.texture_scale, 1.5);
        assert_eq!(ext.blend_sharpness, 8.0);
        assert_eq!(ext.material_properties[0].texture_scale, 2.0);
    }

    #[test]
    #[should_panic(expected = "Albedo texture is required")]
    fn test_builder_missing_albedo() {
        PaletteBuilder::new().add_material_named("test").build();
    }

    #[test]
    fn test_try_build_missing_albedo() {
        let result = PaletteBuilder::new().add_material_named("test").try_build();
        assert!(result.is_none());
    }
}
