//! Material extension for triplanar voxel rendering.

use bevy::ecs::system::{lifetimeless::SRes, SystemParamItem};
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline, StandardMaterial};
use bevy::prelude::*;
use bevy::render::{
    render_asset::RenderAssets,
    render_resource::{
        binding_types::{sampler, storage_buffer_read_only, texture_2d_array, uniform_buffer},
        AsBindGroup, AsBindGroupError, BindGroupEntries, BindGroupLayout,
        BindGroupLayoutEntries, BindGroupLayoutEntry, BindingResources, BufferInitDescriptor,
        BufferUsages, PreparedBindGroup, RenderPipelineDescriptor, SamplerBindingType,
        ShaderStages, ShaderType, SpecializedMeshPipelineError, TextureSampleType,
        UnpreparedBindGroup,
    },
    renderer::RenderDevice,
    texture::{FallbackImage, GpuImage},
};
use bevy::shader::ShaderRef;
use bytemuck::{Pod, Zeroable};

use crate::mesh::{ATTRIBUTE_MATERIAL_IDS, ATTRIBUTE_MATERIAL_WEIGHTS};
use crate::palette::{MaterialPropertiesGpu, TexturePalette, MAX_MATERIALS};

/// Shader asset path.
const TRIPLANAR_SHADER_PATH: &str = "shaders/triplanar_extension.wgsl";

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
#[derive(Asset, TypePath, Clone, Debug)]
pub struct TriplanarExtension {
    /// The texture palette containing all material textures.
    pub palette: Handle<TexturePalette>,

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
            palette: Handle::default(),
            texture_scale: 1.0,
            blend_sharpness: 4.0,
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

    /// Build GPU settings from this extension and palette.
    pub fn build_settings(&self, palette: Option<&TexturePalette>) -> TriplanarSettings {
        let mut flags = 0u32;

        if self.use_biplanar_color {
            flags |= TriplanarSettings::FLAG_USE_BIPLANAR;
        }

        let has_normals = palette.map(|p| p.has_normal_maps()).unwrap_or(false);
        if self.enable_normal_maps && has_normals {
            flags |= TriplanarSettings::FLAG_ENABLE_NORMALS;
        }

        let has_arm = palette.map(|p| p.has_arm()).unwrap_or(false);
        if has_arm {
            flags |= TriplanarSettings::FLAG_HAS_ARM;
        }

        let material_count = palette.map(|p| p.material_count() as u32).unwrap_or(0);

        TriplanarSettings {
            texture_scale: self.texture_scale,
            blend_sharpness: self.blend_sharpness,
            flags,
            material_count,
        }
    }
}

impl AsBindGroup for TriplanarExtension {
    type Data = ();
    type Param = (
        SRes<RenderAssets<GpuImage>>,
        SRes<Assets<TexturePalette>>,
        SRes<FallbackImage>,
    );

    fn as_bind_group(
        &self,
        layout: &BindGroupLayout,
        render_device: &RenderDevice,
        (gpu_images, palettes, fallback_image): &mut SystemParamItem<'_, '_, Self::Param>,
    ) -> Result<PreparedBindGroup, AsBindGroupError> {
        // Get the palette
        let palette = palettes.get(&self.palette);

        // Get albedo texture (required)
        let albedo_handle = palette.map(|p| &p.albedo);
        let albedo_image = albedo_handle
            .and_then(|h| gpu_images.get(h))
            .ok_or(AsBindGroupError::RetryNextUpdate)?;

        // Get optional textures, falling back to 2D array fallback
        let fallback = &fallback_image.d2_array;

        let normal_image = palette
            .and_then(|p| p.normal.as_ref())
            .and_then(|h| gpu_images.get(h));

        let arm_image = palette
            .and_then(|p| p.arm.as_ref())
            .and_then(|h| gpu_images.get(h));

        // Build settings uniform
        let settings = self.build_settings(palette);
        let settings_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("triplanar_settings"),
            contents: bytemuck::bytes_of(&settings),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        // Build material properties storage buffer
        let mut material_props = [MaterialPropertiesGpu::default(); MAX_MATERIALS];
        if let Some(p) = palette {
            for (i, mat) in p.materials.iter().enumerate().take(MAX_MATERIALS) {
                material_props[i] = MaterialPropertiesGpu::from(mat);
            }
        }
        let props_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("triplanar_material_props"),
            contents: bytemuck::cast_slice(&material_props),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        });

        // Create bind group - entries must be in same order as layout entries
        let bind_group = render_device.create_bind_group(
            Some("triplanar_extension_bind_group"),
            layout,
            &BindGroupEntries::sequential((
                // Binding 100: Settings uniform
                settings_buffer.as_entire_binding(),
                // Binding 101: Albedo texture array
                &albedo_image.texture_view,
                // Binding 102: Albedo sampler
                &albedo_image.sampler,
                // Binding 103: Material properties storage
                props_buffer.as_entire_binding(),
                // Binding 104: Normal texture array (or fallback)
                normal_image
                    .map(|i| &i.texture_view)
                    .unwrap_or(&fallback.texture_view),
                // Binding 105: Normal sampler (or fallback)
                normal_image
                    .map(|i| &i.sampler)
                    .unwrap_or(&fallback.sampler),
                // Binding 106: ARM texture array (or fallback)
                arm_image
                    .map(|i| &i.texture_view)
                    .unwrap_or(&fallback.texture_view),
                // Binding 107: ARM sampler (or fallback)
                arm_image.map(|i| &i.sampler).unwrap_or(&fallback.sampler),
            )),
        );

        Ok(PreparedBindGroup {
            bindings: BindingResources(vec![]),
            bind_group,
        })
    }

    fn unprepared_bind_group(
        &self,
        _layout: &BindGroupLayout,
        _render_device: &RenderDevice,
        _param: &mut SystemParamItem<'_, '_, Self::Param>,
        _force_no_bindless: bool,
    ) -> Result<UnpreparedBindGroup, AsBindGroupError> {
        Err(AsBindGroupError::CreateBindGroupDirectly)
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
                (
                    101,
                    texture_2d_array(TextureSampleType::Float { filterable: true }),
                ),
                (102, sampler(SamplerBindingType::Filtering)),
                (
                    103,
                    storage_buffer_read_only::<[MaterialPropertiesGpu; MAX_MATERIALS]>(false),
                ),
                (
                    104,
                    texture_2d_array(TextureSampleType::Float { filterable: true }),
                ),
                (105, sampler(SamplerBindingType::Filtering)),
                (
                    106,
                    texture_2d_array(TextureSampleType::Float { filterable: true }),
                ),
                (107, sampler(SamplerBindingType::Filtering)),
            ),
        )
        .to_vec()
    }

    fn bind_group_data(&self) -> Self::Data {}

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
        // Define the vertex layout including our custom attributes
        let vertex_layout = layout.0.get_layout(&[
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_NORMAL.at_shader_location(1),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(2),
            ATTRIBUTE_MATERIAL_IDS.at_shader_location(3),
            ATTRIBUTE_MATERIAL_WEIGHTS.at_shader_location(4),
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
            .with_normal_maps(false);

        assert_eq!(ext.texture_scale, 2.0);
        assert_eq!(ext.blend_sharpness, 8.0);
        assert!(!ext.use_biplanar_color);
        assert!(!ext.enable_normal_maps);
    }

    #[test]
    fn test_settings_flags_no_palette() {
        let ext = TriplanarExtension::default();
        let settings = ext.build_settings(None);

        assert!(settings.flags & TriplanarSettings::FLAG_USE_BIPLANAR != 0);
        assert!(settings.flags & TriplanarSettings::FLAG_ENABLE_NORMALS == 0);
        assert!(settings.flags & TriplanarSettings::FLAG_HAS_ARM == 0);
        assert_eq!(settings.material_count, 0);
    }

    #[test]
    fn test_settings_scale_and_sharpness() {
        let ext = TriplanarExtension::default()
            .with_texture_scale(2.5)
            .with_blend_sharpness(6.0);

        let settings = ext.build_settings(None);

        assert_eq!(settings.texture_scale, 2.5);
        assert_eq!(settings.blend_sharpness, 6.0);
    }
}
