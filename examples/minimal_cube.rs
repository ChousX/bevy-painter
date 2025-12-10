//! Minimal example showing a single cube with triplanar texturing.
//!
//! This is the simplest possible example to verify the plugin works.

use bevy::prelude::*;
use bevy_painter::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(TriplanarVoxelPlugin)
        .add_systems(Startup, setup)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<TriplanarVoxelMaterial>>,
    asset_server: Res<AssetServer>,
) {
    // Create a simple cube mesh with uniform material
    let mesh = create_simple_cube();
    
    // Load a texture (you'll need to provide an actual texture asset)
    // For testing, you can use Bevy's default icon or any image
    let albedo = asset_server.load("branding/icon.png");
    
    // Build the material with a single material in the palette
    let extension = PaletteBuilder::new()
        .with_albedo(albedo)
        .add_material_named("test_material")
        .build();
    
    let material = TriplanarVoxelMaterial {
        base: StandardMaterial {
            base_color: Color::WHITE,
            ..default()
        },
        extension,
    };
    
    // Spawn the cube
    commands.spawn((
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(material)),
        Transform::from_xyz(0.0, 0.5, 0.0),
    ));
    
    // Add a light
    commands.spawn((
        PointLight {
            intensity: 2000.0,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));
    
    // Spawn camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-3.0, 3.0, 5.0).looking_at(Vec3::new(0.0, 0.5, 0.0), Vec3::Y),
    ));
}

/// Creates a simple 1x1x1 cube centered at origin with material ID 0.
fn create_simple_cube() -> Mesh {
    let mut builder = TriplanarMeshBuilder::new();
    
    // Define cube vertices with positions and normals
    let positions = [
        // Front face (+Z)
        [-0.5, -0.5, 0.5], [0.5, -0.5, 0.5], [0.5, 0.5, 0.5], [-0.5, 0.5, 0.5],
        // Back face (-Z)
        [-0.5, -0.5, -0.5], [-0.5, 0.5, -0.5], [0.5, 0.5, -0.5], [0.5, -0.5, -0.5],
        // Top face (+Y)
        [-0.5, 0.5, -0.5], [-0.5, 0.5, 0.5], [0.5, 0.5, 0.5], [0.5, 0.5, -0.5],
        // Bottom face (-Y)
        [-0.5, -0.5, -0.5], [0.5, -0.5, -0.5], [0.5, -0.5, 0.5], [-0.5, -0.5, 0.5],
        // Right face (+X)
        [0.5, -0.5, -0.5], [0.5, 0.5, -0.5], [0.5, 0.5, 0.5], [0.5, -0.5, 0.5],
        // Left face (-X)
        [-0.5, -0.5, -0.5], [-0.5, -0.5, 0.5], [-0.5, 0.5, 0.5], [-0.5, 0.5, -0.5],
    ];
    
    let normals = [
        [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0],   // Front
        [0.0, 0.0, -1.0], [0.0, 0.0, -1.0], [0.0, 0.0, -1.0], [0.0, 0.0, -1.0], // Back
        [0.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 1.0, 0.0], [0.0, 1.0, 0.0],   // Top
        [0.0, -1.0, 0.0], [0.0, -1.0, 0.0], [0.0, -1.0, 0.0], [0.0, -1.0, 0.0], // Bottom
        [1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 0.0],   // Right
        [-1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], // Left
    ];
    
    // Add all vertices with material ID 0
    for i in 0..24 {
        builder.push_vertex(
            positions[i],
            normals[i],
            VertexMaterialData::single(0), // Use material 0 for everything
        );
    }
    
    // Define triangles (2 per face)
    let indices: Vec<u32> = vec![
        0, 1, 2, 2, 3, 0,       // Front
        4, 5, 6, 6, 7, 4,       // Back
        8, 9, 10, 10, 11, 8,    // Top
        12, 13, 14, 14, 15, 12, // Bottom
        16, 17, 18, 18, 19, 16, // Right
        20, 21, 22, 22, 23, 20, // Left
    ];
    
    builder.push_indices(&indices);
    
    builder.build_unwrap()
}
