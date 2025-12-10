//! Integration test for bevy-sculpter + bevy-painter without external textures.
//!
//! Creates a sphere with procedurally generated materials:
//! - Top hemisphere: "grass" (green)
//! - Bottom hemisphere: "stone" (gray)
//! - Core: "lava" (orange)

use bevy::asset::RenderAssetUsages;
use bevy::mesh::VertexAttributeValues;
use bevy::pbr::ExtendedMaterial;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy_painter::material_field::{
    MaterialBlendSettings, MaterialField, NeighborMaterialFields, compute_vertex_materials,
};
use bevy_painter::mesh::{ATTRIBUTE_MATERIAL_IDS, ATTRIBUTE_MATERIAL_WEIGHTS};
use bevy_painter::prelude::*;
use bevy_sculpter::prelude::*;
use chunky_bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(ChunkyPlugin::default())
        .add_plugins(SurfaceNetsPlugin)
        .add_plugins(TriplanarVoxelPlugin)
        .insert_resource(DensityFieldMeshSize(Vec3::splat(10.0)))
        .init_resource::<MaterialBlendSettings>()
        .add_systems(Startup, setup)
        .add_systems(PostUpdate, inject_material_attributes)
        .add_systems(Update, rotate_camera)
        .run();
}

/// Generates a checkerboard texture layer for visual interest
fn generate_checker_texture(color1: [u8; 4], color2: [u8; 4], checker_size: usize) -> Vec<u8> {
    let size = 64;
    let mut data = Vec::with_capacity(size * size * 4);
    for y in 0..size {
        for x in 0..size {
            let checker = ((x / checker_size) + (y / checker_size)) % 2 == 0;
            let color = if checker { color1 } else { color2 };
            data.extend_from_slice(&color);
        }
    }
    data
}

/// Creates a 2D texture array with procedural materials
fn create_texture_array(images: &mut Assets<Image>) -> Handle<Image> {
    let layer_size = 64;
    let layer_count = 3;

    let grass = generate_checker_texture(
        [34, 139, 34, 255], // Forest green
        [50, 160, 50, 255], // Lighter green
        8,
    );
    let stone = generate_checker_texture(
        [128, 128, 128, 255], // Gray
        [100, 100, 100, 255], // Darker gray
        4,
    );
    let lava = generate_checker_texture(
        [255, 100, 0, 255], // Orange
        [255, 50, 0, 255],  // Red-orange
        8,
    );

    let mut combined_data = Vec::with_capacity(layer_size * layer_size * 4 * layer_count);
    combined_data.extend_from_slice(&grass);
    combined_data.extend_from_slice(&stone);
    combined_data.extend_from_slice(&lava);

    let image = Image::new(
        Extent3d {
            width: layer_size as u32,
            height: layer_size as u32,
            depth_or_array_layers: layer_count as u32,
        },
        TextureDimension::D2,
        combined_data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );

    images.add(image)
}

/// Marker component for chunks that need material attribute injection
#[derive(Component)]
struct NeedsMaterialAttributes;

fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut triplanar_materials: ResMut<Assets<TriplanarVoxelMaterial>>,
) {
    // Create procedural texture array
    let albedo_handle = create_texture_array(&mut images);

    // Create triplanar material
    let triplanar_material = triplanar_materials.add(ExtendedMaterial {
        base: StandardMaterial {
            perceptual_roughness: 0.8,
            ..default()
        },
        extension: TriplanarExtension::new(albedo_handle)
            .with_texture_scale(0.5)
            .with_blend_sharpness(4.0)
            .with_materials(3),
    });

    // Create density field with a sphere
    let mut density_field = DensityField::new();
    let center = Vec3::splat(16.0);
    let radius = 12.0;
    bevy_sculpter::helpers::fill_sphere(&mut density_field, center, radius);

    // Create material field
    let mut material_field = MaterialField::new();
    paint_materials(&mut material_field, center, radius);

    // Spawn chunk - SurfaceNetsPlugin will generate the mesh
    commands.spawn((
        Chunk,
        ChunkPos(IVec3::ZERO),
        density_field,
        material_field,
        DensityFieldDirty,
        NeedsMaterialAttributes,
        MeshMaterial3d(triplanar_material),
        Transform::from_translation(Vec3::splat(-5.0)),
    ));

    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(15.0, 15.0, 15.0).looking_at(Vec3::ZERO, Vec3::Y),
        OrbitCamera::default(),
    ));

    // Lights
    commands.spawn((
        DirectionalLight {
            illuminance: 10000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(10.0, 20.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 300.0,
        ..default()
    });

    info!("Test scene loaded!");
    info!("Materials: 0=grass (green top), 1=stone (gray bottom), 2=lava (orange core)");
}

/// System that injects material attributes into meshes after SurfaceNets generates them
fn inject_material_attributes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    query: Query<
        (
            Entity,
            &Mesh3d,
            &DensityField,
            &MaterialField,
            Option<&NeighborDensityFields>,
            Option<&NeighborMaterialFields>,
        ),
        With<NeedsMaterialAttributes>,
    >,
    mesh_size: Res<DensityFieldMeshSize>,
    blend_settings: Res<MaterialBlendSettings>,
) {
    for (entity, mesh_handle, density, materials, neighbor_density, neighbor_materials) in
        query.iter()
    {
        let Some(mesh) = meshes.get_mut(&mesh_handle.0) else {
            continue;
        };

        // Skip if already has material attributes
        if mesh.attribute(ATTRIBUTE_MATERIAL_IDS).is_some() {
            commands.entity(entity).remove::<NeedsMaterialAttributes>();
            continue;
        }

        // Get vertex positions
        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION).cloned()
        else {
            warn!("Mesh has no positions");
            continue;
        };

        // Compute material data for each vertex
        let mut material_ids: Vec<u32> = Vec::with_capacity(positions.len());
        let mut material_weights: Vec<u32> = Vec::with_capacity(positions.len());

        for pos in positions.iter() {
            let world_pos = Vec3::from_array(*pos);

            let vertex_data = compute_vertex_materials(
                world_pos,
                mesh_size.0,
                density,
                materials,
                neighbor_density,
                neighbor_materials,
                &blend_settings,
            );

            material_ids.push(vertex_data.pack_ids());
            material_weights.push(vertex_data.pack_weights());
        }

        // Insert material attributes
        mesh.insert_attribute(ATTRIBUTE_MATERIAL_IDS, material_ids);
        mesh.insert_attribute(ATTRIBUTE_MATERIAL_WEIGHTS, material_weights);

        // Remove marker
        commands.entity(entity).remove::<NeedsMaterialAttributes>();

        info!(
            "Injected material attributes into mesh with {} vertices",
            positions.len()
        );
    }
}

/// Paints materials based on position relative to sphere center
fn paint_materials(field: &mut MaterialField, center: Vec3, radius: f32) {
    let inner_radius = radius * 0.5;

    for z in 0..32 {
        for y in 0..32 {
            for x in 0..32 {
                let pos = Vec3::new(x as f32, y as f32, z as f32);
                let to_center = pos - center;
                let dist = to_center.length();

                let material = if dist < inner_radius {
                    2 // Inner core: lava
                } else if to_center.y > 0.0 {
                    0 // Upper hemisphere: grass
                } else {
                    1 // Lower hemisphere: stone
                };

                field.set(x, y, z, material);
            }
        }
    }
}

#[derive(Component)]
struct OrbitCamera {
    pub distance: f32,
    pub pitch: f32,
    pub yaw: f32,
    pub speed: f32,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            distance: 20.0,
            pitch: 0.5,
            yaw: 0.0,
            speed: 0.5,
        }
    }
}

fn rotate_camera(time: Res<Time>, mut query: Query<(&mut Transform, &mut OrbitCamera)>) {
    for (mut transform, mut orbit) in query.iter_mut() {
        orbit.yaw += time.delta_secs() * orbit.speed;

        let x = orbit.distance * orbit.pitch.cos() * orbit.yaw.cos();
        let y = orbit.distance * orbit.pitch.sin();
        let z = orbit.distance * orbit.pitch.cos() * orbit.yaw.sin();

        transform.translation = Vec3::new(x, y, z);
        transform.look_at(Vec3::ZERO, Vec3::Y);
    }
}
