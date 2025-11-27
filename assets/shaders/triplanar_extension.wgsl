// Triplanar voxel material extension shader
// Extends StandardMaterial with triplanar mapping and multi-material blending

#import bevy_pbr::{
    mesh_functions,
    view_transformations::position_world_to_clip,
}

// GPU settings
struct TriplanarSettings {
    texture_scale: f32,
    blend_sharpness: f32,
    flags: u32,
    material_count: u32,
}

struct MaterialProperties {
    texture_scale: f32,
    blend_sharpness: f32,
    roughness_override: f32,
    metallic_override: f32,
}

// Bindings - must match extension.rs bind_group_layout_entries
@group(#{MATERIAL_BIND_GROUP}) @binding(100) var<uniform> settings: TriplanarSettings;
@group(#{MATERIAL_BIND_GROUP}) @binding(101) var albedo_array: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(102) var albedo_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(103) var<storage, read> material_props: array<MaterialProperties>;
@group(#{MATERIAL_BIND_GROUP}) @binding(104) var normal_array: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(105) var normal_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(106) var arm_array: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(107) var arm_sampler: sampler;

// Flags
const FLAG_USE_BIPLANAR: u32 = 1u;
const FLAG_ENABLE_NORMALS: u32 = 2u;
const FLAG_HAS_ARM: u32 = 4u;

// Vertex input - locations must match specialize() in extension.rs
struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) material_ids: u32,
    @location(4) material_weights: u32,
}

// Vertex output / Fragment input
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) @interpolate(flat) material_ids: u32,
    @location(4) @interpolate(flat) material_weights: u32,
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;

    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    let world_position = mesh_functions::mesh_position_local_to_world(
        world_from_local,
        vec4<f32>(vertex.position, 1.0)
    );

    out.clip_position = position_world_to_clip(world_position.xyz);
    out.world_position = world_position.xyz;
    out.world_normal = mesh_functions::mesh_normal_local_to_world(
        vertex.normal,
        vertex.instance_index
    );
    out.uv = vertex.uv;

    // Pass material data through (flat interpolation)
    out.material_ids = vertex.material_ids;
    out.material_weights = vertex.material_weights;

    return out;
}

// Utility functions
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

// Sample and blend a single material
fn sample_material(
    world_pos: vec3<f32>,
    world_normal: vec3<f32>,
    material_id: u32,
) -> vec4<f32> {
    let id = min(material_id, settings.material_count - 1u);
    let props = material_props[id];
    let tex_scale = settings.texture_scale * props.texture_scale;
    let sharpness = settings.blend_sharpness * props.blend_sharpness;

    return sample_albedo_triplanar(world_pos, world_normal, id, tex_scale, sharpness);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let world_position = in.world_position;
    let world_normal = normalize(in.world_normal);

    // Unpack material data
    let mat_ids = unpack_material_ids(in.material_ids);
    let mat_weights = unpack_material_weights(in.material_weights);

    // Blend materials
    var final_color = vec4<f32>(0.0);

    if mat_weights.x > 0.001 {
        final_color += sample_material(world_position, world_normal, mat_ids.x) * mat_weights.x;
    }
    if mat_weights.y > 0.001 {
        final_color += sample_material(world_position, world_normal, mat_ids.y) * mat_weights.y;
    }
    if mat_weights.z > 0.001 {
        final_color += sample_material(world_position, world_normal, mat_ids.z) * mat_weights.z;
    }
    if mat_weights.w > 0.001 {
        final_color += sample_material(world_position, world_normal, mat_ids.w) * mat_weights.w;
    }

    // Simple lighting for testing
    let light_dir = normalize(vec3<f32>(1.0, 1.0, 1.0));
    let ndotl = max(dot(world_normal, light_dir), 0.0);
    let ambient = 0.3;
    let lighting = ambient + (1.0 - ambient) * ndotl;

    return vec4<f32>(final_color.rgb * lighting, final_color.a);
}
