//! Simple terrain example demonstrating triplanar voxel materials.
//!
//! This example creates a small terrain mesh with multiple materials
//! blended together using triplanar texture mapping.
//!
//! Run with: `cargo run --example simple_terrain`

use bevy::prelude::*;
use bevy_painter::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(TriplanarVoxelPlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, rotate_camera)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<TriplanarVoxelMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    // Create a procedural texture array for testing
    let albedo_texture = create_test_texture_array(&mut images);

    // Create a terrain mesh
    let mesh = create_terrain_mesh();
    let mesh_handle = meshes.add(mesh);

    // Create the triplanar material
    let material = TriplanarVoxelMaterial {
        base: StandardMaterial {
            ..default()
        },
        extension: TriplanarExtension::new(albedo_texture)
            .with_materials(4)            // 4 materials in texture array
            .with_texture_scale(0.5)      // Larger texture tiling
            .with_blend_sharpness(4.0)    // Sharp triplanar blend
            .with_biplanar_color(false),  // Use full triplanar
    };
    let material_handle = materials.add(material);

    // Spawn the terrain
    commands.spawn((
        Mesh3d(mesh_handle),
        MeshMaterial3d(material_handle),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    // Spawn a light
    commands.spawn((
        DirectionalLight {
            illuminance: 10000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // Ambient light
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 300.0,
        ..default()
    });

    // Spawn camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(8.0, 6.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
        CameraController::default(),
    ));

    // Instructions
    commands.spawn((
        Text::new("Triplanar Voxel Material Test\nCamera orbits automatically"),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        },
    ));
}

/// Create a procedural texture array with 4 colored checker patterns.
fn create_test_texture_array(images: &mut Assets<Image>) -> Handle<Image> {
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

    let size = 64u32;
    let layers = 4u32;
    let checker_size = 8u32;

    // Colors for each layer (RGBA)
    let colors: [[u8; 4]; 4] = [
        [220, 80, 80, 255],   // Red
        [80, 220, 80, 255],   // Green
        [80, 80, 220, 255],   // Blue
        [220, 220, 80, 255],  // Yellow
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
        default(),
    );

    images.add(image)
}

/// Create a simple terrain mesh with varying materials.
fn create_terrain_mesh() -> Mesh {
    let mut builder = TriplanarMeshBuilder::with_capacity(100, 600);

    let grid_size = 8;
    let scale = 1.0;

    // Generate a grid of vertices with height variation
    let mut heights = vec![vec![0.0f32; grid_size + 1]; grid_size + 1];

    // Simple height variation
    for z in 0..=grid_size {
        for x in 0..=grid_size {
            let fx = x as f32 / grid_size as f32;
            let fz = z as f32 / grid_size as f32;

            // Simple rolling hills
            heights[z][x] = (fx * std::f32::consts::PI * 2.0).sin() * 0.3
                + (fz * std::f32::consts::PI * 2.0).cos() * 0.3
                + ((fx + fz) * std::f32::consts::PI).sin() * 0.2;
        }
    }

    // Generate vertices
    for z in 0..=grid_size {
        for x in 0..=grid_size {
            let px = (x as f32 - grid_size as f32 / 2.0) * scale;
            let pz = (z as f32 - grid_size as f32 / 2.0) * scale;
            let py = heights[z][x];

            // Calculate normal from height differences
            let h_l = if x > 0 { heights[z][x - 1] } else { heights[z][x] };
            let h_r = if x < grid_size { heights[z][x + 1] } else { heights[z][x] };
            let h_d = if z > 0 { heights[z - 1][x] } else { heights[z][x] };
            let h_u = if z < grid_size { heights[z + 1][x] } else { heights[z][x] };

            let normal = Vec3::new(h_l - h_r, 2.0 * scale, h_d - h_u).normalize();

            // Determine material based on height and slope
            let slope = 1.0 - normal.y; // 0 = flat, 1 = vertical
            let material_data = if py > 0.3 {
                // High areas: yellow (index 3)
                VertexMaterialData::single(3)
            } else if slope > 0.3 {
                // Steep slopes: blue rock (index 2)
                VertexMaterialData::single(2)
            } else if py < -0.2 {
                // Low areas: blend green and blue (60% green, 40% blue)
                VertexMaterialData::blend2(1, 2, 0.6)
            } else {
                // Mid areas: green grass (index 1) with some red (index 0)
                let red_amount = ((px.abs() + pz.abs()) * 50.0) as u8 % 100;
                if red_amount > 50 {
                    // 80% green, 20% red
                    VertexMaterialData::blend2(1, 0, 0.8)
                } else {
                    VertexMaterialData::single(1)
                }
            };

            builder.push_vertex([px, py, pz], normal.to_array(), material_data);
        }
    }

    // Generate indices
    for z in 0..grid_size {
        for x in 0..grid_size {
            let tl = (z * (grid_size + 1) + x) as u32;
            let tr = tl + 1;
            let bl = tl + (grid_size + 1) as u32;
            let br = bl + 1;

            // Two triangles per quad
            builder.push_triangle(tl, bl, tr);
            builder.push_triangle(tr, bl, br);
        }
    }

    builder.build_unwrap()
}

/// Simple camera controller for orbiting.
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
