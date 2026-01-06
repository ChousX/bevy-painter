// Triplanar voxel material extension shader
// Extends StandardMaterial with triplanar mapping and multi-material blending
// No UVs required - texture coordinates derived from world position

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
    mesh_functions,
    view_transformations::position_world_to_clip,
}

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{FragmentOutput},
    pbr_deferred_functions::deferred_output,
}
#else
#import bevy_pbr::{
    forward_io::{FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
}
#endif

// GPU settings - must match TriplanarSettings in extension.rs
struct TriplanarSettings {
    texture_scale: f32,
    blend_sharpness: f32,
    flags: u32,
    material_count: u32,
}

// Per-material properties - must match MaterialPropertiesGpu in properties.rs
struct MaterialProperties {
    texture_scale: f32,
    blend_sharpness: f32,
    roughness_override: f32,
    metallic_override: f32,
}

// Bindings - must match extension.rs bind_group_layout_entries
// Use #{MATERIAL_BIND_GROUP} placeholder - Bevy replaces this at runtime
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> settings: TriplanarSettings;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var albedo_array: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var albedo_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(103) var<storage, read> material_props: array<MaterialProperties>;
@group(#{MATERIAL_BIND_GROUP}) @binding(104) var normal_array: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(105) var normal_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(106) var arm_array: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(107) var arm_sampler: sampler;

// Flags - must match TriplanarSettings constants
const FLAG_USE_BIPLANAR: u32 = 1u;
const FLAG_ENABLE_NORMALS: u32 = 2u;
const FLAG_HAS_ARM: u32 = 4u;

// Custom vertex input with material attributes
struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) material_ids: u32,
    @location(3) material_weights: u32,
}

// Custom vertex output matching what fragment shader expects
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) @interpolate(flat) material_ids: u32,
    @location(3) @interpolate(flat) material_weights: u32,
    @location(4) instance_index: u32,
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;

    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    let world_position = mesh_functions::mesh_position_local_to_world(
        world_from_local,
        vec4<f32>(vertex.position, 1.0)
    );

    out.position = position_world_to_clip(world_position.xyz);
    out.world_position = world_position;
    out.world_normal = mesh_functions::mesh_normal_local_to_world(
        vertex.normal,
        vertex.instance_index
    );
    out.material_ids = vertex.material_ids;
    out.material_weights = vertex.material_weights;
    out.instance_index = vertex.instance_index;

    return out;
}

// ============================================================================
// Utility functions
// ============================================================================

fn unpack_material_ids(packed: u32) -> vec4<u32> {
    return vec4<u32>(
        packed & 0xFFu,
        (packed >> 8u) & 0xFFu,
        (packed >> 16u) & 0xFFu,
        (packed >> 24u) & 0xFFu,
    );
}

fn unpack_material_weights(packed: u32) -> vec4<f32> {
    let raw = vec4<f32>(
        f32(packed & 0xFFu),
        f32((packed >> 8u) & 0xFFu),
        f32((packed >> 16u) & 0xFFu),
        f32((packed >> 24u) & 0xFFu),
    );
    let sum = raw.x + raw.y + raw.z + raw.w;
    if sum > 0.0 {
        return raw / sum;
    }
    return vec4<f32>(1.0, 0.0, 0.0, 0.0);
}

fn compute_triplanar_weights(world_normal: vec3<f32>, sharpness: f32) -> vec3<f32> {
    var weights = abs(world_normal);
    weights = pow(weights, vec3<f32>(sharpness));
    let sum = weights.x + weights.y + weights.z;
    if sum > 0.0001 {
        return weights / sum;
    }
    return vec3<f32>(0.333, 0.333, 0.334);
}

// ============================================================================
// Triplanar sampling
// ============================================================================

fn sample_albedo_triplanar(
    world_pos: vec3<f32>,
    world_normal: vec3<f32>,
    material_id: u32,
    tex_scale: f32,
    sharpness: f32,
) -> vec4<f32> {
    let weights = compute_triplanar_weights(world_normal, sharpness);

    let uv_x = world_pos.yz * tex_scale;
    let uv_y = world_pos.xz * tex_scale;
    let uv_z = world_pos.xy * tex_scale;

    let col_x = textureSample(albedo_array, albedo_sampler, uv_x, material_id);
    let col_y = textureSample(albedo_array, albedo_sampler, uv_y, material_id);
    let col_z = textureSample(albedo_array, albedo_sampler, uv_z, material_id);

    return col_x * weights.x + col_y * weights.y + col_z * weights.z;
}

fn sample_arm_triplanar(
    world_pos: vec3<f32>,
    world_normal: vec3<f32>,
    material_id: u32,
    tex_scale: f32,
    sharpness: f32,
) -> vec3<f32> {
    let weights = compute_triplanar_weights(world_normal, sharpness);

    let uv_x = world_pos.yz * tex_scale;
    let uv_y = world_pos.xz * tex_scale;
    let uv_z = world_pos.xy * tex_scale;

    let arm_x = textureSample(arm_array, arm_sampler, uv_x, material_id).rgb;
    let arm_y = textureSample(arm_array, arm_sampler, uv_y, material_id).rgb;
    let arm_z = textureSample(arm_array, arm_sampler, uv_z, material_id).rgb;

    return arm_x * weights.x + arm_y * weights.y + arm_z * weights.z;
}

// ============================================================================
// Material sampling
// ============================================================================

struct MaterialSample {
    albedo: vec4<f32>,
    roughness: f32,
    metallic: f32,
    ao: f32,
}

fn sample_material(
    world_pos: vec3<f32>,
    world_normal: vec3<f32>,
    material_id: u32,
) -> MaterialSample {
    var result: MaterialSample;
    
    let id = min(material_id, max(settings.material_count, 1u) - 1u);
    let props = material_props[id];
    
    let tex_scale = settings.texture_scale * props.texture_scale;
    let sharpness = settings.blend_sharpness * props.blend_sharpness;

    result.albedo = sample_albedo_triplanar(world_pos, world_normal, id, tex_scale, sharpness);
    
    if (settings.flags & FLAG_HAS_ARM) != 0u {
        let arm = sample_arm_triplanar(world_pos, world_normal, id, tex_scale, sharpness);
        result.ao = arm.r;
        result.roughness = arm.g;
        result.metallic = arm.b;
    } else {
        result.ao = 1.0;
        result.roughness = 0.5;
        result.metallic = 0.0;
    }
    
    if props.roughness_override >= 0.0 {
        result.roughness = props.roughness_override;
    }
    if props.metallic_override >= 0.0 {
        result.metallic = props.metallic_override;
    }
    
    return result;
}

// ============================================================================
// Fragment shader - manually construct PbrInput since we have custom VertexOutput
// ============================================================================

#import bevy_pbr::{
    pbr_types::{PbrInput, pbr_input_new},
    pbr_functions as fns,
    mesh_view_bindings::view,
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    let world_position = in.world_position.xyz;
    let world_normal = normalize(in.world_normal);

    // Unpack material data
    let mat_ids = unpack_material_ids(in.material_ids);
    let mat_weights = unpack_material_weights(in.material_weights);

    // Blend materials
    var blended_albedo = vec4<f32>(0.0);
    var blended_roughness = 0.0;
    var blended_metallic = 0.0;
    var blended_ao = 0.0;

    if mat_weights.x > 0.001 {
        let sample = sample_material(world_position, world_normal, mat_ids.x);
        blended_albedo += sample.albedo * mat_weights.x;
        blended_roughness += sample.roughness * mat_weights.x;
        blended_metallic += sample.metallic * mat_weights.x;
        blended_ao += sample.ao * mat_weights.x;
    }
    if mat_weights.y > 0.001 {
        let sample = sample_material(world_position, world_normal, mat_ids.y);
        blended_albedo += sample.albedo * mat_weights.y;
        blended_roughness += sample.roughness * mat_weights.y;
        blended_metallic += sample.metallic * mat_weights.y;
        blended_ao += sample.ao * mat_weights.y;
    }
    if mat_weights.z > 0.001 {
        let sample = sample_material(world_position, world_normal, mat_ids.z);
        blended_albedo += sample.albedo * mat_weights.z;
        blended_roughness += sample.roughness * mat_weights.z;
        blended_metallic += sample.metallic * mat_weights.z;
        blended_ao += sample.ao * mat_weights.z;
    }
    if mat_weights.w > 0.001 {
        let sample = sample_material(world_position, world_normal, mat_ids.w);
        blended_albedo += sample.albedo * mat_weights.w;
        blended_roughness += sample.roughness * mat_weights.w;
        blended_metallic += sample.metallic * mat_weights.w;
        blended_ao += sample.ao * mat_weights.w;
    }

    // Build PbrInput manually (following array_texture.wgsl pattern)
    var pbr_input: PbrInput = pbr_input_new();
    
    // Set material base color
    pbr_input.material.base_color = blended_albedo;
    
    // Geometry setup
    pbr_input.frag_coord = in.position;
    pbr_input.world_position = in.world_position;
    pbr_input.world_normal = fns::prepare_world_normal(
        world_normal,
        false,  // double_sided
        is_front,
    );
    pbr_input.is_orthographic = view.clip_from_view[3].w == 1.0;
    pbr_input.N = normalize(pbr_input.world_normal);
    pbr_input.V = fns::calculate_view(in.world_position, pbr_input.is_orthographic);

#ifdef PREPASS_PIPELINE
    let out = deferred_output(in, pbr_input);
#else
    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif

    return out;
}
