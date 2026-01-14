//! Visibility terrain example demonstrating fog-of-war style culling.
//!
//! This example creates terrain with a VisibilityField that controls
//! which parts of the mesh are rendered. An expanding sphere reveals
//! terrain over time, simulating exploration/fog-of-war.
//!
//! Controls:
//! - Space: Reset visibility (hide all terrain)
//! - R: Reveal all terrain instantly
//!
//! Run with: `cargo run --example visibility_terrain`

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy_painter::material_field::{MaterialBlendSettings, MaterialField, VisibilityField};
use bevy_painter::mesh::{
    ATTRIBUTE_MATERIAL_IDS, ATTRIBUTE_MATERIAL_WEIGHTS, ATTRIBUTE_VISIBILITY,
};
use bevy_painter::prelude::*;
use bevy_sculpter::field::Field; // Import Field trait for get/set methods

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(TriplanarVoxelPlugin)
        .init_resource::<MaterialBlendSettings>()
        .init_resource::<RevealState>()
        .add_systems(Startup, setup)
        .add_systems(Update, (rotate_camera, animate_reveal, handle_input))
        .run();
}

/// Tracks the expanding reveal sphere
#[derive(Resource)]
struct RevealState {
    center: Vec3,
    current_radius: f32,
    max_radius: f32,
    speed: f32,
    active: bool,
}

impl Default for RevealState {
    fn default() -> Self {
        Self {
            center: Vec3::ZERO,
            current_radius: 0.0,
            max_radius: 12.0,
            speed: 3.0,
            active: true,
        }
    }
}

/// Marker for our terrain entity
#[derive(Component)]
struct Terrain;

/// Stored terrain data for rebuilding mesh
#[derive(Component)]
struct TerrainData {
    heights: Vec<Vec<f32>>,
    grid_size: usize,
    scale: f32,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<TriplanarVoxelMaterial>>,
    mut images: ResMut<Assets<Image>>,
    blend_settings: Res<MaterialBlendSettings>,
) {
    // Create procedural texture array
    let albedo_texture = create_test_texture_array(&mut images);

    // Create terrain data
    let grid_size = 16;
    let scale = 0.5;
    let heights = generate_heights(grid_size);

    // Create material and visibility fields
    let material_field = create_material_field(&heights, grid_size);
    let visibility_field = VisibilityField::new_hidden(); // Start fully hidden

    // Build initial mesh (all hidden)
    let mesh = build_terrain_mesh(
        &heights,
        grid_size,
        scale,
        &material_field,
        &visibility_field,
        &blend_settings,
    );
    let mesh_handle = meshes.add(mesh);

    // Create triplanar material WITH visibility culling enabled
    let material = TriplanarVoxelMaterial {
        base: StandardMaterial::default(),
        extension: TriplanarExtension::new(albedo_texture)
            .with_materials(4)
            .with_texture_scale(0.5)
            .with_blend_sharpness(4.0)
            .with_visibility_culling(0.5), // Discard fragments < 50% visible
    };
    let material_handle = materials.add(material);

    // Spawn terrain
    commands.spawn((
        Mesh3d(mesh_handle),
        MeshMaterial3d(material_handle),
        Transform::default(),
        Terrain,
        TerrainData {
            heights,
            grid_size,
            scale,
        },
        material_field,
        visibility_field,
    ));

    // Light
    commands.spawn((
        DirectionalLight {
            illuminance: 10000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.spawn(AmbientLight {
        color: Color::WHITE,
        brightness: 300.0,
        ..default()
    });

    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(8.0, 6.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
        CameraController::default(),
    ));

    // UI
    commands.spawn((
        Text::new("Visibility Terrain Demo\n\nTerrain reveals from center over time\n\nControls:\n  Space - Reset (hide all)\n  R - Reveal all\n  Camera orbits automatically"),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        },
    ));
}

/// Generate height data for terrain
fn generate_heights(grid_size: usize) -> Vec<Vec<f32>> {
    let mut heights = vec![vec![0.0f32; grid_size + 1]; grid_size + 1];

    for z in 0..=grid_size {
        for x in 0..=grid_size {
            let fx = x as f32 / grid_size as f32;
            let fz = z as f32 / grid_size as f32;

            // Rolling hills
            heights[z][x] = (fx * std::f32::consts::PI * 2.0).sin() * 0.4
                + (fz * std::f32::consts::PI * 2.0).cos() * 0.4
                + ((fx + fz) * std::f32::consts::PI * 1.5).sin() * 0.25;
        }
    }

    heights
}

/// Create material field based on height/slope
fn create_material_field(heights: &[Vec<f32>], grid_size: usize) -> MaterialField {
    let mut field = MaterialField::new();

    // We'll use a simplified approach - store material per grid cell
    // In a real voxel game you'd have a full 3D field
    for z in 0..grid_size {
        for x in 0..grid_size {
            let h = heights[z][x];
            let material = if h > 0.3 {
                3 // Yellow - peaks
            } else if h > 0.0 {
                1 // Green - mid
            } else if h > -0.3 {
                2 // Blue - low
            } else {
                0 // Red - valleys
            };

            // Store in first layer of field (we're using 2D terrain)
            field.set(x as u32, 0, z as u32, material);
        }
    }

    field
}

/// Build terrain mesh with material and visibility data
fn build_terrain_mesh(
    heights: &[Vec<f32>],
    grid_size: usize,
    scale: f32,
    material_field: &MaterialField,
    visibility_field: &VisibilityField,
    _blend_settings: &MaterialBlendSettings,
) -> Mesh {
    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut material_ids: Vec<u32> = Vec::new();
    let mut material_weights: Vec<u32> = Vec::new();
    let mut visibility: Vec<f32> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    // Generate vertices
    for z in 0..=grid_size {
        for x in 0..=grid_size {
            let px = (x as f32 - grid_size as f32 / 2.0) * scale;
            let pz = (z as f32 - grid_size as f32 / 2.0) * scale;
            let py = heights[z][x];

            // Calculate normal
            let h_l = if x > 0 {
                heights[z][x - 1]
            } else {
                heights[z][x]
            };
            let h_r = if x < grid_size {
                heights[z][x + 1]
            } else {
                heights[z][x]
            };
            let h_d = if z > 0 {
                heights[z - 1][x]
            } else {
                heights[z][x]
            };
            let h_u = if z < grid_size {
                heights[z + 1][x]
            } else {
                heights[z][x]
            };
            let normal = Vec3::new(h_l - h_r, 2.0 * scale, h_d - h_u).normalize();

            positions.push([px, py, pz]);
            normals.push(normal.to_array());

            // Get material from field
            let mat_x = x.min(31) as u32;
            let mat_z = z.min(31) as u32;
            let mat_id = material_field.get(mat_x, 0, mat_z);
            let mat_data = VertexMaterialData::single(mat_id);

            material_ids.push(mat_data.pack_ids());
            material_weights.push(mat_data.pack_weights());

            // Get visibility from field
            let vis = visibility_field.get(mat_x, 0, mat_z) as f32 / 255.0;
            visibility.push(vis);
        }
    }

    // Generate indices
    for z in 0..grid_size {
        for x in 0..grid_size {
            let tl = (z * (grid_size + 1) + x) as u32;
            let tr = tl + 1;
            let bl = tl + (grid_size + 1) as u32;
            let br = bl + 1;

            indices.extend_from_slice(&[tl, bl, tr, tr, bl, br]);
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );

    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(ATTRIBUTE_MATERIAL_IDS, material_ids);
    mesh.insert_attribute(ATTRIBUTE_MATERIAL_WEIGHTS, material_weights);
    mesh.insert_attribute(ATTRIBUTE_VISIBILITY, visibility);
    mesh.insert_indices(Indices::U32(indices));

    mesh
}

/// Animate the expanding reveal sphere
fn animate_reveal(
    time: Res<Time>,
    mut reveal_state: ResMut<RevealState>,
    mut terrain_q: Query<
        (&TerrainData, &MaterialField, &mut VisibilityField, &Mesh3d),
        With<Terrain>,
    >,
    mut meshes: ResMut<Assets<Mesh>>,
    blend_settings: Res<MaterialBlendSettings>,
) {
    if !reveal_state.active {
        return;
    }

    // Expand radius over time
    reveal_state.current_radius += reveal_state.speed * time.delta_secs();

    if reveal_state.current_radius > reveal_state.max_radius {
        reveal_state.current_radius = reveal_state.max_radius;
        reveal_state.active = false;
    }

    let Ok((terrain_data, material_field, mut visibility_field, mesh_handle)) =
        terrain_q.single_mut()
    else {
        return;
    };

    // Update visibility field based on distance from center
    let grid_size = terrain_data.grid_size;
    let scale = terrain_data.scale;
    let radius = reveal_state.current_radius;
    let center = reveal_state.center;

    for z in 0..=grid_size {
        for x in 0..=grid_size {
            let px = (x as f32 - grid_size as f32 / 2.0) * scale;
            let pz = (z as f32 - grid_size as f32 / 2.0) * scale;
            let py = terrain_data.heights[z][x];

            let pos = Vec3::new(px, py, pz);
            let dist = pos.distance(center);

            // Soft edge - fade visibility near the boundary
            let vis = if dist < radius - 0.5 {
                255u8
            } else if dist < radius + 0.5 {
                ((1.0 - (dist - (radius - 0.5))) * 255.0) as u8
            } else {
                0u8
            };

            let field_x = x.min(31) as u32;
            let field_z = z.min(31) as u32;
            visibility_field.set(field_x, 0, field_z, vis);
        }
    }

    // Rebuild mesh with updated visibility
    let new_mesh = build_terrain_mesh(
        &terrain_data.heights,
        terrain_data.grid_size,
        terrain_data.scale,
        material_field,
        &visibility_field,
        &blend_settings,
    );

    if let Some(mesh) = meshes.get_mut(&mesh_handle.0) {
        *mesh = new_mesh;
    }
}

/// Handle keyboard input
fn handle_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut reveal_state: ResMut<RevealState>,
    mut terrain_q: Query<
        (&TerrainData, &MaterialField, &mut VisibilityField, &Mesh3d),
        With<Terrain>,
    >,
    mut meshes: ResMut<Assets<Mesh>>,
    blend_settings: Res<MaterialBlendSettings>,
) {
    let Ok((terrain_data, material_field, mut visibility_field, mesh_handle)) =
        terrain_q.single_mut()
    else {
        return;
    };

    let mut needs_rebuild = false;

    // Space - reset to hidden
    if keyboard.just_pressed(KeyCode::Space) {
        *visibility_field = VisibilityField::new_hidden();
        reveal_state.current_radius = 0.0;
        reveal_state.active = true;
        needs_rebuild = true;
    }

    // R - reveal all
    if keyboard.just_pressed(KeyCode::KeyR) {
        *visibility_field = VisibilityField::new_visible();
        reveal_state.active = false;
        needs_rebuild = true;
    }

    if needs_rebuild {
        let new_mesh = build_terrain_mesh(
            &terrain_data.heights,
            terrain_data.grid_size,
            terrain_data.scale,
            material_field,
            &visibility_field,
            &blend_settings,
        );

        if let Some(mesh) = meshes.get_mut(&mesh_handle.0) {
            *mesh = new_mesh;
        }
    }
}

/// Create procedural texture array with 4 colored checker patterns
fn create_test_texture_array(images: &mut Assets<Image>) -> Handle<Image> {
    let size = 64u32;
    let layers = 4u32;
    let checker_size = 8u32;

    let colors: [[u8; 4]; 4] = [
        [220, 80, 80, 255],  // Red
        [80, 220, 80, 255],  // Green
        [80, 80, 220, 255],  // Blue
        [220, 220, 80, 255], // Yellow
    ];

    let dark_factor = 0.6;
    let mut data = Vec::with_capacity((size * size * layers * 4) as usize);

    for layer in 0..layers {
        let base_color = colors[layer as usize];
        let dark_color = [
            (base_color[0] as f32 * dark_factor) as u8,
            (base_color[1] as f32 * dark_factor) as u8,
            (base_color[2] as f32 * dark_factor) as u8,
            255,
        ];

        for y in 0..size {
            for x in 0..size {
                let checker = ((x / checker_size) + (y / checker_size)) % 2 == 0;
                let color = if checker { base_color } else { dark_color };
                data.extend_from_slice(&color);
            }
        }
    }

    let image = Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: layers,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );

    images.add(image)
}

#[derive(Component)]
struct CameraController {
    radius: f32,
    speed: f32,
    height: f32,
}

impl Default for CameraController {
    fn default() -> Self {
        Self {
            radius: 10.0,
            speed: 0.3,
            height: 5.0,
        }
    }
}

fn rotate_camera(time: Res<Time>, mut query: Query<(&mut Transform, &CameraController)>) {
    for (mut transform, controller) in &mut query {
        let angle = time.elapsed_secs() * controller.speed;
        let x = angle.cos() * controller.radius;
        let z = angle.sin() * controller.radius;

        transform.translation = Vec3::new(x, controller.height, z);
        transform.look_at(Vec3::ZERO, Vec3::Y);
    }
}
