//! Interactive 3D material painting example.
//!
//! Paint materials onto voxel terrain in real-time with triplanar texturing.
//!
//! Controls:
//! - Middle click + drag: Rotate camera
//! - Left click (hold): Paint current material
//! - 1-4: Select material (1=grass, 2=stone, 3=lava, 4=water)
//! - Scroll wheel: Adjust brush size
//! - [ / ]: Adjust brush strength (blend sharpness)
//! - WASD/Space/Shift: Move camera

use bevy::{
    asset::RenderAssetUsages,
    input::mouse::{MouseMotion, MouseWheel},
    mesh::{Indices, PrimitiveTopology, VertexAttributeValues},
    pbr::ExtendedMaterial,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    window::PrimaryWindow,
};
use bevy_painter::{
    material_field::{
        MaterialBlendSettings, MaterialField, MaterialSlice, MaterialSliceExt,
        NeighborMaterialFields, compute_vertex_materials,
    },
    mesh::{ATTRIBUTE_MATERIAL_IDS, ATTRIBUTE_MATERIAL_WEIGHTS},
    prelude::*,
};
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
        .init_resource::<PaintBrush>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                fly_camera,
                paint_materials,
                update_brush_preview,
                select_material,
                ui_text,
            ),
        )
        .add_systems(PostUpdate, (
            gather_neighbor_materials,
            rebuild_material_meshes,
        ).chain())
        .run();
}

// =============================================================================
// Resources
// =============================================================================

#[derive(Resource)]
struct PaintBrush {
    radius: f32,
    min_radius: f32,
    max_radius: f32,
    current_material: u8,
    material_names: [&'static str; 4],
    material_colors: [Color; 4],
}

impl Default for PaintBrush {
    fn default() -> Self {
        Self {
            radius: 3.0,
            min_radius: 1.0,
            max_radius: 10.0,
            current_material: 0,
            material_names: ["Grass", "Stone", "Lava", "Water"],
            material_colors: [
                Color::srgb(0.2, 0.8, 0.2),  // Green
                Color::srgb(0.5, 0.5, 0.5),  // Gray
                Color::srgb(1.0, 0.4, 0.0),  // Orange
                Color::srgb(0.1, 0.5, 1.0),  // Blue
            ],
        }
    }
}

#[derive(Resource)]
struct SharedTriplanarMaterial(Handle<TriplanarVoxelMaterial>);

// =============================================================================
// Components
// =============================================================================

/// Marker for chunks that need material mesh rebuilding
#[derive(Component)]
struct MaterialMeshDirty;

/// Marker for chunks that have been initialized with triplanar material
#[derive(Component)]
struct HasTriplanarMaterial;

#[derive(Component)]
struct BrushPreview;

#[derive(Component)]
struct UiText;

#[derive(Component)]
struct FlyCam {
    speed: f32,
    sensitivity: f32,
    pitch: f32,
    yaw: f32,
}

impl Default for FlyCam {
    fn default() -> Self {
        Self {
            speed: 20.0,
            sensitivity: 0.003,
            pitch: -0.3,
            yaw: 0.8,
        }
    }
}

// =============================================================================
// Setup
// =============================================================================

fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut triplanar_materials: ResMut<Assets<TriplanarVoxelMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut standard_materials: ResMut<Assets<StandardMaterial>>,
) {
    // Create texture array
    let albedo_handle = create_texture_array(&mut images);

    // Create and store triplanar material
    let triplanar_material = triplanar_materials.add(ExtendedMaterial {
        base: StandardMaterial {
            perceptual_roughness: 0.8,
            ..default()
        },
        extension: TriplanarExtension::new(albedo_handle)
            .with_texture_scale(0.3)
            .with_blend_sharpness(4.0)
            .with_materials(4),
    });
    commands.insert_resource(SharedTriplanarMaterial(triplanar_material));

    // Spawn a 3x3x3 grid of chunks
    for x in -1..=1 {
        for y in -1..=1 {
            for z in -1..=1 {
                let mut density_field = DensityField::new();
                let mut material_field = MaterialField::new();

                // Create sphere SDF
                let local_center = Vec3::splat(16.0);
                let global_offset = Vec3::new(x as f32, y as f32, z as f32) * 32.0;
                let sphere_center = Vec3::ZERO;
                let local_sphere_center = sphere_center - global_offset + local_center;

                bevy_sculpter::helpers::fill_sphere(&mut density_field, local_sphere_center, 24.0);

                // Initialize materials based on height (Y position in world space)
                for mz in 0..32 {
                    for my in 0..32 {
                        for mx in 0..32 {
                            let world_y = (y * 32 + my as i32) as f32 - 16.0;
                            let material = if world_y > 8.0 {
                                0 // Grass on top
                            } else if world_y > -8.0 {
                                1 // Stone in middle
                            } else {
                                2 // Lava at bottom
                            };
                            material_field.set(mx, my, mz, material);
                        }
                    }
                }

                commands.spawn((
                    Chunk,
                    ChunkPos(IVec3::new(x, y, z)),
                    density_field,
                    material_field,
                    DensityFieldDirty,
                    MaterialMeshDirty,
                ));
            }
        }
    }

    // Brush preview sphere
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(1.0).mesh().ico(2).unwrap())),
        MeshMaterial3d(standard_materials.add(StandardMaterial {
            base_color: Color::srgba(0.2, 0.8, 0.2, 0.3),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        })),
        Transform::from_scale(Vec3::ZERO),
        BrushPreview,
    ));

    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(35.0, 25.0, 35.0).looking_at(Vec3::ZERO, Vec3::Y),
        FlyCam::default(),
    ));

    // Lights
    commands.spawn((
        DirectionalLight {
            illuminance: 10000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(10.0, 30.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 400.0,
        ..default()
    });

    // UI
    commands.spawn((
        Text::new(""),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        },
        UiText,
    ));

    info!("Paint example loaded!");
    info!("Use number keys 1-4 to select material, left click to paint");
}

fn create_texture_array(images: &mut Assets<Image>) -> Handle<Image> {
    let layer_size = 64usize;
    let layer_count = 4usize;

    let generate_checker = |color1: [u8; 4], color2: [u8; 4], checker_size: usize| -> Vec<u8> {
        let mut data = Vec::with_capacity(layer_size * layer_size * 4);
        for y in 0..layer_size {
            for x in 0..layer_size {
                let checker = ((x / checker_size) + (y / checker_size)) % 2 == 0;
                let color = if checker { color1 } else { color2 };
                data.extend_from_slice(&color);
            }
        }
        data
    };

    let grass = generate_checker([34, 139, 34, 255], [50, 160, 50, 255], 8);
    let stone = generate_checker([128, 128, 128, 255], [100, 100, 100, 255], 4);
    let lava = generate_checker([255, 100, 0, 255], [255, 50, 0, 255], 8);
    let water = generate_checker([30, 144, 255, 255], [0, 100, 200, 255], 8);

    let mut combined = Vec::with_capacity(layer_size * layer_size * 4 * layer_count);
    combined.extend_from_slice(&grass);
    combined.extend_from_slice(&stone);
    combined.extend_from_slice(&lava);
    combined.extend_from_slice(&water);

    images.add(Image::new(
        Extent3d {
            width: layer_size as u32,
            height: layer_size as u32,
            depth_or_array_layers: layer_count as u32,
        },
        TextureDimension::D2,
        combined,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    ))
}

// =============================================================================
// Camera
// =============================================================================

fn fly_camera(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: EventReader<MouseMotion>,
    mut scroll: EventReader<MouseWheel>,
    mut query: Query<(&mut Transform, &mut FlyCam)>,
    mut brush: ResMut<PaintBrush>,
) {
    let Ok((mut transform, mut fly_cam)) = query.single_mut() else {
        return;
    };

    // Camera rotation with middle mouse
    if mouse_buttons.pressed(MouseButton::Middle) {
        for motion in mouse_motion.read() {
            fly_cam.yaw -= motion.delta.x * fly_cam.sensitivity;
            fly_cam.pitch -= motion.delta.y * fly_cam.sensitivity;
            fly_cam.pitch = fly_cam.pitch.clamp(-1.5, 1.5);
        }
        transform.rotation = Quat::from_euler(EulerRot::YXZ, fly_cam.yaw, fly_cam.pitch, 0.0);
    } else {
        mouse_motion.clear();
    }

    // Brush size with scroll
    for ev in scroll.read() {
        brush.radius = (brush.radius + ev.y * 0.5).clamp(brush.min_radius, brush.max_radius);
    }

    // Movement
    let mut velocity = Vec3::ZERO;
    let forward = transform.forward();
    let right = transform.right();

    if keyboard.pressed(KeyCode::KeyW) { velocity += *forward; }
    if keyboard.pressed(KeyCode::KeyS) { velocity -= *forward; }
    if keyboard.pressed(KeyCode::KeyA) { velocity -= *right; }
    if keyboard.pressed(KeyCode::KeyD) { velocity += *right; }
    if keyboard.pressed(KeyCode::Space) { velocity += Vec3::Y; }
    if keyboard.pressed(KeyCode::ShiftLeft) { velocity -= Vec3::Y; }

    let speed = if keyboard.pressed(KeyCode::ControlLeft) {
        fly_cam.speed * 3.0
    } else {
        fly_cam.speed
    };

    if velocity.length_squared() > 0.0 {
        velocity = velocity.normalize() * speed * time.delta_secs();
        transform.translation += velocity;
    }
}

// =============================================================================
// Material Selection
// =============================================================================

fn select_material(keyboard: Res<ButtonInput<KeyCode>>, mut brush: ResMut<PaintBrush>) {
    if keyboard.just_pressed(KeyCode::Digit1) { brush.current_material = 0; }
    if keyboard.just_pressed(KeyCode::Digit2) { brush.current_material = 1; }
    if keyboard.just_pressed(KeyCode::Digit3) { brush.current_material = 2; }
    if keyboard.just_pressed(KeyCode::Digit4) { brush.current_material = 3; }
}

// =============================================================================
// Painting
// =============================================================================

fn paint_materials(
    mut commands: Commands,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<FlyCam>>,
    mut chunks: Query<(Entity, &ChunkPos, &DensityField, &mut MaterialField)>,
    mesh_size: Res<DensityFieldMeshSize>,
    brush: Res<PaintBrush>,
    chunk_manager: Res<ChunkManager>,
) {
    if !mouse_buttons.pressed(MouseButton::Left) {
        return;
    }

    let Ok(window) = window_q.single() else { return };
    let Some(cursor_pos) = window.cursor_position() else { return };
    let Ok((camera, cam_transform)) = camera_q.single() else { return };
    let Ok(ray) = camera.viewport_to_world(cam_transform, cursor_pos) else { return };

    let Some(hit_point) = raycast_terrain(&chunks, &mesh_size, ray) else { return };

    let chunk_world_size = mesh_size.0;
    let world_brush_radius = brush.radius;

    // How close to boundary before we need to update neighbors (in grid units)
    // This should match the sampling radius used in compute_vertex_materials
    const BOUNDARY_MARGIN: f32 = 2.0;

    for (entity, chunk_pos, _density, mut material_field) in chunks.iter_mut() {
        let chunk_world_origin = chunk_pos.0.as_vec3() * chunk_world_size;
        let local_hit = hit_point - chunk_world_origin;

        let scale = Vec3::splat(32.0) / chunk_world_size;
        let grid_center = local_hit * scale;
        let grid_radius = world_brush_radius * scale.x;

        // AABB check
        let brush_min = grid_center - Vec3::splat(grid_radius);
        let brush_max = grid_center + Vec3::splat(grid_radius);

        if brush_max.x < 0.0 || brush_min.x > 32.0
            || brush_max.y < 0.0 || brush_min.y > 32.0
            || brush_max.z < 0.0 || brush_min.z > 32.0
        {
            continue;
        }

        // Paint sphere
        let grid_radius_sq = grid_radius * grid_radius;
        let min = brush_min.max(Vec3::ZERO).as_ivec3();
        let max = brush_max.min(Vec3::splat(31.0)).as_ivec3();

        let mut painted = false;
        for z in min.z..=max.z {
            for y in min.y..=max.y {
                for x in min.x..=max.x {
                    let pos = Vec3::new(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5);
                    if pos.distance_squared(grid_center) <= grid_radius_sq {
                        material_field.set(x as u32, y as u32, z as u32, brush.current_material);
                        painted = true;
                    }
                }
            }
        }

        if painted {
            commands.entity(entity).insert(MaterialMeshDirty);

            // Mark neighboring chunks as dirty if we painted near their boundary
            // This ensures vertices that sample across boundaries get updated
            let neighbors_to_update = [
                (brush_min.x < BOUNDARY_MARGIN, IVec3::new(-1, 0, 0)),
                (brush_max.x > 32.0 - BOUNDARY_MARGIN, IVec3::new(1, 0, 0)),
                (brush_min.y < BOUNDARY_MARGIN, IVec3::new(0, -1, 0)),
                (brush_max.y > 32.0 - BOUNDARY_MARGIN, IVec3::new(0, 1, 0)),
                (brush_min.z < BOUNDARY_MARGIN, IVec3::new(0, 0, -1)),
                (brush_max.z > 32.0 - BOUNDARY_MARGIN, IVec3::new(0, 0, 1)),
            ];

            for (near_boundary, offset) in neighbors_to_update {
                if near_boundary {
                    let neighbor_pos = chunk_pos.0 + offset;
                    if let Some(neighbor_entity) = chunk_manager.get_chunk(&neighbor_pos) {
                        commands.entity(neighbor_entity).insert(MaterialMeshDirty);
                    }
                }
            }
        }
    }
}

fn raycast_terrain(
    chunks: &Query<(Entity, &ChunkPos, &DensityField, &mut MaterialField)>,
    mesh_size: &DensityFieldMeshSize,
    ray: Ray3d,
) -> Option<Vec3> {
    let chunk_world_size = mesh_size.0;
    let max_dist = 200.0;
    let step = 0.1;
    let mut t = 0.0;

    while t < max_dist {
        let point = ray.origin + ray.direction * t;
        let chunk_coord = (point / chunk_world_size).floor().as_ivec3();

        for (_entity, chunk_pos, field, _mat) in chunks.iter() {
            if chunk_pos.0 != chunk_coord {
                continue;
            }

            let chunk_origin = chunk_pos.0.as_vec3() * chunk_world_size;
            let local_pos = point - chunk_origin;
            let scale = Vec3::splat(32.0) / chunk_world_size;
            let grid_pos = local_pos * scale;

            if grid_pos.cmpge(Vec3::ZERO).all() && grid_pos.cmplt(Vec3::splat(32.0)).all() {
                let density = field.get(grid_pos.x as u32, grid_pos.y as u32, grid_pos.z as u32);
                if density < 0.0 {
                    return Some(point);
                }
            }
        }
        t += step;
    }
    None
}

// =============================================================================
// Neighbor Material Gathering
// =============================================================================

fn gather_neighbor_materials(
    mut commands: Commands,
    dirty_chunks: Query<(Entity, &ChunkPos), With<MaterialMeshDirty>>,
    all_materials: Query<&MaterialField>,
    chunk_manager: Res<ChunkManager>,
) {
    use bevy_painter::material_field::NeighborFace;

    for (entity, chunk_pos) in dirty_chunks.iter() {
        let mut neighbors = NeighborMaterialFields::default();

        for face in NeighborFace::ALL {
            let neighbor_pos = chunk_pos.0 + face.offset();

            if let Some(neighbor_entity) = chunk_manager.get_chunk(&neighbor_pos) {
                if let Ok(neighbor_field) = all_materials.get(neighbor_entity) {
                    neighbors.neighbors[face as usize] =
                        Some(MaterialSlice::from_material_field(neighbor_field, face));
                }
            }
        }

        commands.entity(entity).insert(neighbors);
    }
}

// =============================================================================
// Mesh Rebuilding
// =============================================================================

fn rebuild_material_meshes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    query: Query<(
        Entity,
        &Mesh3d,
        &DensityField,
        &MaterialField,
        Option<&NeighborDensityFields>,
        Option<&NeighborMaterialFields>,
        Option<&HasTriplanarMaterial>,
    ), With<MaterialMeshDirty>>,
    mesh_size: Res<DensityFieldMeshSize>,
    blend_settings: Res<MaterialBlendSettings>,
    triplanar_material: Option<Res<SharedTriplanarMaterial>>,
) {
    let Some(triplanar_material) = triplanar_material else { return };

    for (entity, mesh_handle, density, materials, neighbor_density, neighbor_materials, has_triplanar) in query.iter() {
        let Some(mesh) = meshes.get(&mesh_handle.0) else { continue };

        let Some(VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else { continue };

        let Some(VertexAttributeValues::Float32x3(normals)) =
            mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
        else { continue };

        let indices = mesh.indices().map(|i| match i {
            Indices::U16(v) => v.iter().map(|&i| i as u32).collect::<Vec<_>>(),
            Indices::U32(v) => v.clone(),
        });

        let positions = positions.clone();
        let normals = normals.clone();

        // Compute material data
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

        // Create new mesh
        let mut new_mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
        );

        new_mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        new_mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        new_mesh.insert_attribute(ATTRIBUTE_MATERIAL_IDS, material_ids);
        new_mesh.insert_attribute(ATTRIBUTE_MATERIAL_WEIGHTS, material_weights);

        if let Some(indices) = indices {
            new_mesh.insert_indices(Indices::U32(indices));
        }

        let new_mesh_handle = meshes.add(new_mesh);

        let mut entity_commands = commands.entity(entity);
        entity_commands
            .remove::<MaterialMeshDirty>()
            .insert(Mesh3d(new_mesh_handle));

        // Only apply triplanar material once (first time)
        if has_triplanar.is_none() {
            entity_commands
                .remove::<MeshMaterial3d<StandardMaterial>>()
                .insert((
                    MeshMaterial3d(triplanar_material.0.clone()),
                    HasTriplanarMaterial,
                ));
        }
    }
}

// =============================================================================
// Brush Preview
// =============================================================================

fn update_brush_preview(
    window_q: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<FlyCam>>,
    chunks: Query<(Entity, &ChunkPos, &DensityField, &mut MaterialField)>,
    mesh_size: Res<DensityFieldMeshSize>,
    brush: Res<PaintBrush>,
    mut preview_q: Query<(&mut Transform, &MeshMaterial3d<StandardMaterial>), With<BrushPreview>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Ok((mut preview_transform, mat_handle)) = preview_q.single_mut() else { return };
    let Ok(window) = window_q.single() else { return };
    let Some(cursor_pos) = window.cursor_position() else {
        preview_transform.scale = Vec3::ZERO;
        return;
    };
    let Ok((camera, cam_transform)) = camera_q.single() else { return };
    let Ok(ray) = camera.viewport_to_world(cam_transform, cursor_pos) else { return };

    if let Some(hit) = raycast_terrain(&chunks, &mesh_size, ray) {
        preview_transform.translation = hit;
        preview_transform.scale = Vec3::splat(brush.radius);

        if let Some(mat) = materials.get_mut(&mat_handle.0) {
            mat.base_color = brush.material_colors[brush.current_material as usize]
                .with_alpha(0.4);
        }
    } else {
        preview_transform.scale = Vec3::ZERO;
    }
}

// =============================================================================
// UI
// =============================================================================

fn ui_text(brush: Res<PaintBrush>, mut text_q: Query<&mut Text, With<UiText>>) {
    let Ok(mut text) = text_q.single_mut() else { return };

    let material_list: String = (0..4)
        .map(|i| {
            let marker = if i == brush.current_material { ">" } else { " " };
            format!("{} {}: {}", marker, i + 1, brush.material_names[i as usize])
        })
        .collect::<Vec<_>>()
        .join("\n");

    *text = Text::new(format!(
        "Paint Controls:\n\
         Middle Click + Drag: Rotate camera\n\
         Left Click (hold): Paint material\n\
         WASD/Space/Shift: Move camera\n\
         Scroll: Brush size ({:.1})\n\
         Ctrl: Speed boost\n\
         \n\
         Materials (press 1-4):\n\
         {}\n",
        brush.radius, material_list
    ));
}
