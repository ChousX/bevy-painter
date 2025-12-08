//! Material extension for triplanar voxel rendering.

use bevy::ecs::system::{lifetimeless::SRes, SystemParamItem};
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::render::render_resource::{OwnedBindingResource, TextureViewDimension};
use bevy::pbr::{
    ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline,
    StandardMaterial,
};
use bevy::prelude::*;
use bevy::render::{
    render_asset::RenderAssets,
    render_resource::{
        binding_types::{sampler, storage_buffer_read_only, texture_2d_array, uniform_buffer},
        AsBindGroup, AsBindGroupError, BindGroupEntries, BindGroupLayout, BindGroupLayoutEntries,
        BindGroupLayoutEntry, BindingResources, BufferInitDescriptor, BufferUsages,
        PreparedBindGroup, RenderPipelineDescriptor, SamplerBindingType, ShaderStages, ShaderType,
        SpecializedMeshPipelineError, TextureSampleType, UnpreparedBindGroup,
    },
    renderer::RenderDevice,
    texture::{FallbackImage, GpuImage},
};
use bevy::shader::ShaderRef;
use bytemuck::{Pod, Zeroable};

use crate::mesh::{ATTRIBUTE_MATERIAL_IDS, ATTRIBUTE_MATERIAL_WEIGHTS};
use crate::palette::{MaterialPropertiesGpu, MAX_MATERIALS};

/// Shader asset path (embedded).
const TRIPLANAR_SHADER_PATH: &str = "embedded://bevy_painter/material/shaders/triplanar_extension.wgsl";
/// Convenience type alias for the complete triplanar voxel material.
pub type TriplanarVoxelMaterial = ExtendedMaterial<StandardMaterial, TriplanarExtension>;

/// GPU-side settings for triplanar rendering.
#[derive(Clone, Copy, Debug, Default, ShaderType, Pod, Zeroable)]
#[repr(C)]
pub struct TriplanarSettings {
    /// Global texture scale multiplier.
    pub texture_scale: f32,
    /// Global blend sharpness multiplier.
    pub blend_sharpness: f32,
    /// Flags for shader features.
    pub flags: u32,
    /// Number of materials in the palette.
    pub material_count: u32,
}

impl TriplanarSettings {
    pub const FLAG_USE_BIPLANAR: u32 = 1 << 0;
    pub const FLAG_ENABLE_NORMALS: u32 = 1 << 1;
    pub const FLAG_HAS_ARM: u32 = 1 << 2;
}

/// Material extension that adds triplanar mapping and multi-material blending.
///
/// This extension stores texture handles directly rather than referencing a palette,
/// making it self-contained for the render world.
#[derive(Asset, TypePath, Clone, Debug)]
pub struct TriplanarExtension {
    /// Albedo texture array (required).
    pub albedo: Handle<Image>,

    /// Normal map texture array (optional).
    pub normal: Option<Handle<Image>>,

    /// ARM (AO/Roughness/Metallic) texture array (optional).
    pub arm: Option<Handle<Image>>,

    /// Per-material properties.
    pub material_properties: Vec<MaterialPropertiesGpu>,

    /// Global texture scale multiplier (default: 1.0).
    pub texture_scale: f32,

    /// Global blend sharpness multiplier (default: 4.0).
    pub blend_sharpness: f32,

    /// Use biplanar mapping for color textures (default: true).
    pub use_biplanar_color: bool,

    /// Enable triplanar normal mapping (default: true).
    pub enable_normal_maps: bool,
}

impl Default for TriplanarExtension {
    fn default() -> Self {
        Self {
            albedo: Handle::default(),
            normal: None,
            arm: None,
            material_properties: Vec::new(),
            texture_scale: 1.0,
            blend_sharpness: 4.0,
            use_biplanar_color: true,
            enable_normal_maps: true,
        }
    }
}

impl TriplanarExtension {
    /// Create a new triplanar extension with an albedo texture array.
    pub fn new(albedo: Handle<Image>) -> Self {
        Self {
            albedo,
            ..default()
        }
    }

    /// Set the normal map texture array.
    pub fn with_normal(mut self, normal: Handle<Image>) -> Self {
        self.normal = Some(normal);
        self
    }

    /// Set the ARM texture array.
    pub fn with_arm(mut self, arm: Handle<Image>) -> Self {
        self.arm = Some(arm);
        self
    }

    /// Set material properties.
    pub fn with_material_properties(mut self, properties: Vec<MaterialPropertiesGpu>) -> Self {
        self.material_properties = properties;
        self
    }

    /// Add a material with default properties.
    pub fn with_material(mut self) -> Self {
        self.material_properties.push(MaterialPropertiesGpu::default());
        self
    }

    /// Add multiple materials with default properties.
    pub fn with_materials(mut self, count: usize) -> Self {
        for _ in 0..count {
            self.material_properties.push(MaterialPropertiesGpu::default());
        }
        self
    }

    /// Set the global texture scale.
    pub fn with_texture_scale(mut self, scale: f32) -> Self {
        self.texture_scale = scale;
        self
    }

    /// Set the global blend sharpness.
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

    /// Build GPU settings from this extension.
    pub fn build_settings(&self) -> TriplanarSettings {
        let mut flags = 0u32;

        if self.use_biplanar_color {
            flags |= TriplanarSettings::FLAG_USE_BIPLANAR;
        }

        if self.enable_normal_maps && self.normal.is_some() {
            flags |= TriplanarSettings::FLAG_ENABLE_NORMALS;
        }

        if self.arm.is_some() {
            flags |= TriplanarSettings::FLAG_HAS_ARM;
        }

        TriplanarSettings {
            texture_scale: self.texture_scale,
            blend_sharpness: self.blend_sharpness,
            flags,
            material_count: self.material_properties.len().max(1) as u32,
        }
    }
}

impl AsBindGroup for TriplanarExtension {
    type Data = ();
    type Param = (SRes<RenderAssets<GpuImage>>, SRes<FallbackImage>);

    fn bind_group_data(&self) -> Self::Data {}

    fn unprepared_bind_group(
        &self,
        _layout: &BindGroupLayout,
        render_device: &RenderDevice,
        (gpu_images, fallback_image): &mut SystemParamItem<'_, '_, Self::Param>,
        _force_no_bindless: bool,
    ) -> Result<UnpreparedBindGroup, AsBindGroupError> {
        use bevy::render::render_resource::OwnedBindingResource;

        // Get albedo texture (required)
        let albedo_image = gpu_images
            .get(&self.albedo)
            .ok_or(AsBindGroupError::RetryNextUpdate)?;

        // Get optional textures, falling back to 2D array fallback
        let fallback = &fallback_image.d2_array;

        let normal_image = self.normal.as_ref().and_then(|h| gpu_images.get(h));
        let arm_image = self.arm.as_ref().and_then(|h| gpu_images.get(h));

        // Build settings uniform
        let settings = self.build_settings();
        let settings_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("triplanar_settings"),
            contents: bytemuck::bytes_of(&settings),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        // Build material properties storage buffer
        let mut material_props = [MaterialPropertiesGpu::default(); MAX_MATERIALS];
        for (i, props) in self.material_properties.iter().enumerate().take(MAX_MATERIALS) {
            material_props[i] = *props;
        }
        if self.material_properties.is_empty() {
            material_props[0] = MaterialPropertiesGpu::default();
        }

        let props_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("triplanar_material_props"),
            contents: bytemuck::cast_slice(&material_props),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        });

        Ok(UnpreparedBindGroup {
    bindings: BindingResources(vec![
        (100, OwnedBindingResource::Buffer(settings_buffer)),
        (101, OwnedBindingResource::TextureView(
            TextureViewDimension::D2Array,
            albedo_image.texture_view.clone()
        )),
        (102, OwnedBindingResource::Sampler(
            SamplerBindingType::Filtering,
            albedo_image.sampler.clone()
        )),
        (103, OwnedBindingResource::Buffer(props_buffer)),
        (104, OwnedBindingResource::TextureView(
            TextureViewDimension::D2Array,
            normal_image.map(|i| i.texture_view.clone())
                .unwrap_or_else(|| fallback.texture_view.clone())
        )),
        (105, OwnedBindingResource::Sampler(
            SamplerBindingType::Filtering,
            normal_image.map(|i| i.sampler.clone())
                .unwrap_or_else(|| fallback.sampler.clone())
        )),
        (106, OwnedBindingResource::TextureView(
            TextureViewDimension::D2Array,
            arm_image.map(|i| i.texture_view.clone())
                .unwrap_or_else(|| fallback.texture_view.clone())
        )),
        (107, OwnedBindingResource::Sampler(
            SamplerBindingType::Filtering,
            arm_image.map(|i| i.sampler.clone())
                .unwrap_or_else(|| fallback.sampler.clone())
        )),
    ]),
})
    }

    fn bind_group_layout_entries(
        _render_device: &RenderDevice,
        _force_no_bindless: bool,
    ) -> Vec<BindGroupLayoutEntry>
    where
        Self: Sized,
    {
        BindGroupLayoutEntries::with_indices(
            ShaderStages::VERTEX_FRAGMENT,
            (
                (100, uniform_buffer::<TriplanarSettings>(false)),
                (101, texture_2d_array(TextureSampleType::Float { filterable: true })),
                (102, sampler(SamplerBindingType::Filtering)),
                (103, storage_buffer_read_only::<[MaterialPropertiesGpu; MAX_MATERIALS]>(false)),
                (104, texture_2d_array(TextureSampleType::Float { filterable: true })),
                (105, sampler(SamplerBindingType::Filtering)),
                (106, texture_2d_array(TextureSampleType::Float { filterable: true })),
                (107, sampler(SamplerBindingType::Filtering)),
            ),
        )
        .to_vec()
    }

    fn label() -> Option<&'static str> {
        Some("triplanar_extension")
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

    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        // Define the vertex layout - only what we need for triplanar (no UVs!)
        let vertex_layout = layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_NORMAL.at_shader_location(1),
            ATTRIBUTE_MATERIAL_IDS.at_shader_location(2),
            ATTRIBUTE_MATERIAL_WEIGHTS.at_shader_location(3),
        ])?;

        descriptor.vertex.buffers = vec![vertex_layout];

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_extension() {
        let ext = TriplanarExtension::default();

        assert_eq!(ext.texture_scale, 1.0);
        assert_eq!(ext.blend_sharpness, 4.0);
        assert!(ext.use_biplanar_color);
        assert!(ext.enable_normal_maps);
    }

    #[test]
    fn test_extension_builder() {
        let ext = TriplanarExtension::new(Handle::default())
            .with_texture_scale(2.0)
            .with_blend_sharpness(8.0)
            .with_biplanar_color(false)
            .with_normal_maps(false)
            .with_materials(4);

        assert_eq!(ext.texture_scale, 2.0);
        assert_eq!(ext.blend_sharpness, 8.0);
        assert!(!ext.use_biplanar_color);
        assert!(!ext.enable_normal_maps);
        assert_eq!(ext.material_properties.len(), 4);
    }

    #[test]
    fn test_settings_flags() {
        let ext = TriplanarExtension::default();
        let settings = ext.build_settings();

        assert!(settings.flags & TriplanarSettings::FLAG_USE_BIPLANAR != 0);
        assert!(settings.flags & TriplanarSettings::FLAG_ENABLE_NORMALS == 0); // No normal texture
        assert!(settings.flags & TriplanarSettings::FLAG_HAS_ARM == 0);
    }

    #[test]
    fn test_settings_scale_and_sharpness() {
        let ext = TriplanarExtension::default()
            .with_texture_scale(2.5)
            .with_blend_sharpness(6.0);

        let settings = ext.build_settings();

        assert_eq!(settings.texture_scale, 2.5);
        assert_eq!(settings.blend_sharpness, 6.0);
    }
}
