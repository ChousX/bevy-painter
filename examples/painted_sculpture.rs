//! Example combining bevy-sculptor (SDF sculpting) with bevy-painter (triplanar textures).
//!
//! This demonstrates:
//! - Sculpting terrain with density fields
//! - Using MaterialField for per-voxel material storage
//! - Automatic density-weighted material blending at vertices
//! - Rendering with triplanar multi-material textures
//!
//! Controls:
//! - Right mouse button (hold): Add material
//! - Left mouse button (hold): Remove material
//! - 1-4 keys: Select paint material
//! - Shift + click: Paint material (without sculpting)
//! - Scroll wheel: Adjust brush size
//! - Middle mouse + drag: Orbit camera

use bevy::{
    input::mouse::{AccumulatedMouseMotion, MouseWheel},
    prelude::*,
    window::PrimaryWindow,
};
use bevy_painter::prelude::*;
use bevy_sculpter::prelude::*;
use chunky_bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(ChunkyPlugin::default())
        .add_plugins(TriplanarVoxelPlugin)
        .insert_resource(DensityFieldMeshSize(vec3(10., 10., 10.)))
        .init_resource::<Brush>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                gather_neighbor_materials,
                sculpt_terrain,
                paint_terrain,
                remesh_dirty_chunks,
                orbit_camera,
                adjust_brush,
                select_material,
            ),
        )
        .run();
}

#[derive(Resource)]
struct Brush {
    radius: f32,
    strength: f32,
    falloff: f32,
    paint_material: u8,
}

impl Default for Brush {
    fn default() -> Self {
        Self {
            radius: 2.0,
            strength: 5.0,
            falloff: 2.0,
            paint_material: 0,
        }
    }
}

#[derive(Resource)]
struct TerrainMaterial(Handle<TriplanarVoxelMaterial>);

fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<TriplanarVoxelMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let albedo = create_material_textures(&mut images);

    let material = TriplanarVoxelMaterial {
        base: StandardMaterial::default(),
        extension: TriplanarExtension::new(albedo)
            .with_materials(4)
            .with_texture_scale(0.5)
            .with_blend_sharpness(4.0)
            .with_biplanar_color(false),
    };
    let mat_handle = materials.add(material);
    commands.insert_resource(TerrainMaterial(mat_handle.clone()));

    let mesh_size = vec3(10., 10., 10.);
    let terrain_config = TerrainMaterialConfig {
        grass_material: 0,
        dirt_material: 2,
        steep_material: 1,
        snow_material: 3,
        snow_height: 8.0,
        dirt_height: -5.0,
        steep_threshold: 0.6,
    };

    // Spawn terrain chunks
    for x in -1..=1 {
        for y in -1..=1 {
            for z in -1..=1 {
                let chunk_pos = ivec3(x, y, z);
                
                // Create density field with sphere
                let mut density = DensityField::new();
                let local_center = vec3(16.0, 16.0, 16.0);
                let global_offset = chunk_pos.as_vec3() * 32.0;
                let sphere_center = vec3(0.0, -5.0, 0.0);
                let local_sphere_center = sphere_center - global_offset + local_center;
                bevy_sculpter::helpers::fill_sphere(&mut density, local_sphere_center, 25.0);

                // Create material field with natural terrain assignment
                let mut mat_field = MaterialField::new();
                fill_terrain_natural(
                    &mut mat_field,
                    &density,
                    chunk_pos,
                    mesh_size,
                    &terrain_config,
                );

                commands.spawn((
                    Chunk,
                    ChunkPos(chunk_pos),
                    density,
                    mat_field,
                    DensityFieldDirty,
                    MeshMaterial3d(mat_handle.clone()),
                    Transform::from_translation(chunk_pos.as_vec3() * mesh_size),
                ));
            }
        }
    }

    // Lighting
    commands.spawn((
        DirectionalLight {
            illuminance: 15000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(5.0, 10.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 400.0,
        ..default()
    });

    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(20.0, 15.0, 20.0).looking_at(Vec3::ZERO, Vec3::Y),
        OrbitCamera {
            distance: 35.0,
            pitch: 0.4,
            yaw: 0.785,
        },
    ));

    // UI
    commands.spawn((
        Text::new(
            "Painted Sculpture Example\n\
             RMB: Add | LMB: Remove | Scroll: Brush Size\n\
             Shift+Click: Paint Only | 1-4: Select Material",
        ),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        },
    ));
}//! Example combining bevy-sculptor (SDF sculpting) with bevy-painter (triplanar textures).
//!
//! This demonstrates:
//! - Sculpting terrain with density fields
//! - Post-processing meshes to add material blend data
//! - Rendering with triplanar multi-material textures
//!
//! Controls:
//! - Right mouse button (hold): Add material
//! - Left mouse button (hold): Remove material  
//! - Scroll wheel: Adjust brush size
//! - Middle mouse + drag: Orbit camera

use bevy::{
    input::mouse::{AccumulatedMouseMotion, MouseWheel},
    prelude::*,
    window::PrimaryWindow,
};
use bevy_painter::prelude::*;
use bevy_sculpter::prelude::*;
use chunky_bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(ChunkyPlugin::default())
        .add_plugins(TriplanarVoxelPlugin)
        // Note: We DON'T add SurfaceNetsPlugin - we handle meshing ourselves
        // to inject material attributes
        .insert_resource(DensityFieldMeshSize(vec3(10., 10., 10.)))
        .init_resource::<Brush>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                sculpt_terrain,
                remesh_dirty_chunks,
                orbit_camera,
                adjust_brush,
            ),
        )
        .run();
}

#[derive(Resource)]
struct Brush {
    radius: f32,
    strength: f32,
    falloff: f32,
}

impl Default for Brush {
    fn default() -> Self {
        Self {
            radius: 2.0,
            strength: 5.0,
            falloff: 2.0,
        }
    }
}

#[derive(Resource)]
struct TerrainMaterial(Handle<TriplanarVoxelMaterial>);

fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<TriplanarVoxelMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    // Create procedural texture array (4 materials: grass, rock, dirt, snow)
    let albedo = create_material_textures(&mut images);

    // Create triplanar material with 4 terrain types
    let material = TriplanarVoxelMaterial {
        base: StandardMaterial::default(),
        extension: TriplanarExtension::new(albedo)
            .with_materials(4)
            .with_texture_scale(0.5)
            .with_blend_sharpness(4.0)
            .with_biplanar_color(false),
    };
    let mat_handle = materials.add(material);
    commands.insert_resource(TerrainMaterial(mat_handle.clone()));

    // Spawn initial terrain chunks (3x3x3 grid)
    let mesh_size = vec3(10., 10., 10.);
    for x in -1..=1 {
        for y in -1..=1 {
            for z in -1..=1 {
                let mut field = DensityField::new();
                
                // Create terrain: sphere at origin + some noise
                let local_center = vec3(16.0, 16.0, 16.0);
                let global_offset = vec3(x as f32, y as f32, z as f32) * 32.0;
                let sphere_center = vec3(0.0, -5.0, 0.0); // Slightly below origin
                let local_sphere_center = sphere_center - global_offset + local_center;
                bevy_sculpter::helpers::fill_sphere(&mut field, local_sphere_center, 25.0);

                commands.spawn((
                    Chunk,
                    ChunkPos(ivec3(x, y, z)),
                    field,
                    DensityFieldDirty,
                    MeshMaterial3d(mat_handle.clone()),
                    Transform::from_translation(ivec3(x, y, z).as_vec3() * mesh_size),
                ));
            }
        }
    }

    // Lighting
    commands.spawn((
        DirectionalLight {
            illuminance: 15000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(5.0, 10.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 400.0,
        ..default()
    });

    // Camera with orbit controller
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(20.0, 15.0, 20.0).looking_at(Vec3::ZERO, Vec3::Y),
        OrbitCamera { distance: 35.0, pitch: 0.4, yaw: 0.785 },
    ));

    // UI
    commands.spawn((
        Text::new("Painted Sculpture Example\nRMB: Add | LMB: Remove | Scroll: Brush Size"),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        },
    ));
}

/// Meshing system that uses MaterialField for vertex material computation
fn remesh_dirty_chunks(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mesh_size: Res<DensityFieldMeshSize>,
    chunks: Query<
        (
            Entity,
            &DensityField,
            &MaterialField,
            Option<&NeighborDensityFields>,
            Option<&NeighborMaterialFields>,
        ),
        With<DensityFieldDirty>,
    >,
) {
    for (entity, density, materials, density_neighbors, material_neighbors) in chunks.iter() {
        let dn = density_neighbors.cloned().unwrap_or_default();

        // Generate base mesh using surface nets
        let Some(mut mesh) = bevy_sculpter::mesher::generate_mesh_cpu(density, &dn, mesh_size.0)
        else {
            commands
                .entity(entity)
                .remove::<(Mesh3d, DensityFieldDirty)>();
            continue;
        };

        // Extract positions for material computation
        let positions: Vec<[f32; 3]> = mesh
            .attribute(Mesh::ATTRIBUTE_POSITION)
            .and_then(|attr| attr.as_float3())
            .map(|p| p.to_vec())
            .unwrap_or_default();

        // Compute material attributes using density-weighted blending
        let (ids, weights) = compute_vertex_materials(
            &positions,
            density,
            materials,
            density_neighbors,
            material_neighbors,
            mesh_size.0,
        );

        mesh.insert_attribute(ATTRIBUTE_MATERIAL_IDS, ids);
        mesh.insert_attribute(ATTRIBUTE_MATERIAL_WEIGHTS, weights);

        let handle = meshes.add(mesh);
        commands
            .entity(entity)
            .insert(Mesh3d(handle))
            .remove::<DensityFieldDirty>();
    }
}

// === Material Selection ===

fn select_material(keyboard: Res<ButtonInput<KeyCode>>, mut brush: ResMut<Brush>) {
    if keyboard.just_pressed(KeyCode::Digit1) {
        brush.paint_material = 0;
    } else if keyboard.just_pressed(KeyCode::Digit2) {
        brush.paint_material = 1;
    } else if keyboard.just_pressed(KeyCode::Digit3) {
        brush.paint_material = 2;
    } else if keyboard.just_pressed(KeyCode::Digit4) {
        brush.paint_material = 3;
    }
}

// === Sculpting ===

fn sculpt_terrain(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    window: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
    time: Res<Time>,
    brush: Res<Brush>,
    mesh_size: Res<DensityFieldMeshSize>,
    mut chunks: Query<(Entity, &ChunkPos, &mut DensityField)>,
) {
    // Skip if shift is held (paint-only mode)
    if keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight) {
        return;
    }

    let adding = buttons.pressed(MouseButton::Right);
    let removing = buttons.pressed(MouseButton::Left);
    if !adding && !removing {
        return;
    }

    let Ok(window) = window.single() else { return };
    let Some(cursor) = window.cursor_position() else { return };
    let Ok((camera, cam_tf)) = camera_q.single() else { return };
    let Ok(ray) = camera.viewport_to_world(cam_tf, cursor) else { return };

    let Some(hit) = raycast_terrain(&chunks, &mesh_size, ray) else { return };

    let chunk_size = mesh_size.0;
    let rate = if adding { -brush.strength } else { brush.strength };

    let mut modified = Vec::new();

    for (entity, chunk_pos, mut field) in chunks.iter_mut() {
        let chunk_origin = chunk_pos.0.as_vec3() * chunk_size;
        let local_hit = hit - chunk_origin;

        let scale = Vec3::splat(32.0) / chunk_size;
        let grid_center = local_hit * scale;
        let grid_radius = brush.radius * scale.x;

        if !aabb_intersects_sphere(Vec3::ZERO, Vec3::splat(32.0), grid_center, grid_radius) {
            continue;
        }

        bevy_sculpter::helpers::brush_smooth_timed(
            &mut field,
            grid_center,
            grid_radius,
            rate,
            time.delta_secs(),
            brush.falloff,
        );
        modified.push(entity);
    }

    for entity in modified {
        commands.entity(entity).insert(DensityFieldDirty);
    }
}

/// Paint-only mode (Shift + click)
fn paint_terrain(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    window: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform)>,
    brush: Res<Brush>,
    mesh_size: Res<DensityFieldMeshSize>,
    mut chunks: Query<(Entity, &ChunkPos, &DensityField, &mut MaterialField)>,
) {
    // Only paint when shift is held
    if !keyboard.pressed(KeyCode::ShiftLeft) && !keyboard.pressed(KeyCode::ShiftRight) {
        return;
    }

    if !buttons.pressed(MouseButton::Left) && !buttons.pressed(MouseButton::Right) {
        return;
    }

    let Ok(window) = window.single() else { return };
    let Some(cursor) = window.cursor_position() else { return };
    let Ok((camera, cam_tf)) = camera_q.single() else { return };
    let Ok(ray) = camera.viewport_to_world(cam_tf, cursor) else { return };

    // Borrow density fields immutably for raycast
    let density_chunks: Vec<_> = chunks
        .iter()
        .map(|(e, pos, d, _)| (e, pos.clone(), d.clone()))
        .collect();

    let raycast_query: Vec<_> = density_chunks
        .iter()
        .map(|(e, pos, d)| (*e, pos, d))
        .collect();

    let Some(hit) = raycast_terrain_vec(&raycast_query, &mesh_size, ray) else { return };

    let chunk_size = mesh_size.0;
    let mut modified = Vec::new();

    for (entity, chunk_pos, density, mut mat_field) in chunks.iter_mut() {
        let chunk_origin = chunk_pos.0.as_vec3() * chunk_size;
        let local_hit = hit - chunk_origin;

        let scale = Vec3::splat(32.0) / chunk_size;
        let grid_center = local_hit * scale;
        let grid_radius = brush.radius * scale.x;

        if !aabb_intersects_sphere(Vec3::ZERO, Vec3::splat(32.0), grid_center, grid_radius) {
            continue;
        }

        // Paint only near surface
        paint_surface(
            &mut mat_field,
            density,
            grid_center,
            grid_radius,
            brush.paint_material,
            2.0, // surface threshold
        );
        modified.push(entity);
    }

    for entity in modified {
        commands.entity(entity).insert(DensityFieldDirty);
    }
}

fn raycast_terrain(
    chunks: &Query<(Entity, &ChunkPos, &mut DensityField)>,
    mesh_size: &DensityFieldMeshSize,
    ray: Ray3d,
) -> Option<Vec3> {
    let chunk_size = mesh_size.0;
    let max_dist = 100.0;
    let step = 0.1;
    let mut t = 0.0;

    while t < max_dist {
        let point = ray.origin + ray.direction * t;
        let chunk_coord = (point / chunk_size).floor().as_ivec3();

        for (_, pos, field) in chunks.iter() {
            if pos.0 != chunk_coord {
                continue;
            }

            let origin = pos.0.as_vec3() * chunk_size;
            let local = point - origin;
            let scale = Vec3::splat(32.0) / chunk_size;
            let grid = local * scale;

            if grid.cmpge(Vec3::ZERO).all() && grid.cmplt(Vec3::splat(32.0)).all() {
                let d = field.get(grid.x as u32, grid.y as u32, grid.z as u32);
                if d < 0.0 {
                    return Some(point);
                }
            }
        }
        t += step;
    }
    None
}

fn raycast_terrain_vec(
    chunks: &[(&Entity, &ChunkPos, &DensityField)],
    mesh_size: &DensityFieldMeshSize,
    ray: Ray3d,
) -> Option<Vec3> {
    let chunk_size = mesh_size.0;
    let max_dist = 100.0;
    let step = 0.1;
    let mut t = 0.0;

    while t < max_dist {
        let point = ray.origin + ray.direction * t;
        let chunk_coord = (point / chunk_size).floor().as_ivec3();

        for (_, pos, field) in chunks.iter() {
            if pos.0 != chunk_coord {
                continue;
            }

            let origin = pos.0.as_vec3() * chunk_size;
            let local = point - origin;
            let scale = Vec3::splat(32.0) / chunk_size;
            let grid = local * scale;

            if grid.cmpge(Vec3::ZERO).all() && grid.cmplt(Vec3::splat(32.0)).all() {
                let d = field.get(grid.x as u32, grid.y as u32, grid.z as u32);
                if d < 0.0 {
                    return Some(point);
                }
            }
        }
        t += step;
    }
    None
}

fn aabb_intersects_sphere(min: Vec3, max: Vec3, center: Vec3, radius: f32) -> bool {
    let closest = center.clamp(min, max);
    center.distance_squared(closest) <= radius * radius
}

// === Camera & Input ===

#[derive(Component)]
struct OrbitCamera {
    distance: f32,
    pitch: f32,
    yaw: f32,
}

fn orbit_camera(
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    mut cam: Query<(&mut Transform, &mut OrbitCamera)>,
) {
    if !buttons.pressed(MouseButton::Middle) { return; }

    let Ok((mut tf, mut orbit)) = cam.single_mut() else { return };
    let delta = motion.delta;

    orbit.yaw -= delta.x * 0.005;
    orbit.pitch = (orbit.pitch - delta.y * 0.005).clamp(-1.4, 1.4);

    let rot = Quat::from_euler(EulerRot::YXZ, orbit.yaw, orbit.pitch, 0.0);
    let pos = rot * Vec3::new(0.0, 0.0, orbit.distance);
    *tf = Transform::from_translation(pos).looking_at(Vec3::ZERO, Vec3::Y);
}

fn adjust_brush(mut scroll: EventReader<MouseWheel>, mut brush: ResMut<Brush>) {
    for ev in scroll.read() {
        brush.radius = (brush.radius + ev.y * 0.3).clamp(0.5, 8.0);
    }
}

// === Procedural Textures ===

fn create_material_textures(images: &mut Assets<Image>) -> Handle<Image> {
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

    let size = 64;
    let layers = 4;
    let mut data = vec![0u8; size * size * layers * 4];

    // Colors for each layer: grass, rock, dirt, snow
    let colors: [[u8; 3]; 4] = [
        [80, 140, 60],   // Grass - green
        [120, 110, 100], // Rock - gray
        [140, 100, 60],  // Dirt - brown
        [240, 245, 250], // Snow - white
    ];

    for layer in 0..layers {
        for y in 0..size {
            for x in 0..size {
                let idx = ((layer * size + y) * size + x) * 4;
                let c = colors[layer];

                // Add subtle checker pattern for visual interest
                let checker = ((x / 8 + y / 8) % 2 == 0) as i32;
                let variation = (checker * 10 - 5) as i8;

                data[idx] = c[0].saturating_add_signed(variation);
                data[idx + 1] = c[1].saturating_add_signed(variation);
                data[idx + 2] = c[2].saturating_add_signed(variation);
                data[idx + 3] = 255;
            }
        }
    }

    images.add(Image::new(
        Extent3d {
            width: size as u32,
            height: size as u32,
            depth_or_array_layers: layers as u32,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        default(),
    ))
}
