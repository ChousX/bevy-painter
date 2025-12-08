//! Example combining bevy-sculptor with bevy-painter's MaterialField.
//!
//! Controls:
//! - RMB (hold): Add material
//! - LMB (hold): Remove material
//! - Shift + click: Paint only (no sculpting)
//! - 1-4: Select paint material
//! - Scroll: Brush size
//! - MMB + drag: Orbit camera

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
        .add_systems(Update, (sculpt, paint, remesh, orbit_camera, input))
        .run();
}

#[derive(Resource, Default)]
struct Brush {
    radius: f32,
    strength: f32,
    material: u8,
}

impl Brush {
    fn new() -> Self {
        Self { radius: 2.0, strength: 5.0, material: 0 }
    }
}

#[derive(Resource)]
struct TerrainMat(Handle<TriplanarVoxelMaterial>);

fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<TriplanarVoxelMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    commands.insert_resource(Brush::new());

    // Create texture array
    let albedo = create_textures(&mut images);
    let mat = materials.add(TriplanarVoxelMaterial {
        base: StandardMaterial::default(),
        extension: TriplanarExtension::new(albedo)
            .with_materials(4)
            .with_texture_scale(0.5)
            .with_blend_sharpness(4.0),
    });
    commands.insert_resource(TerrainMat(mat.clone()));

    let mesh_size = vec3(10., 10., 10.);

    // Spawn chunks
    for x in -1..=1 {
        for y in -1..=1 {
            for z in -1..=1 {
                let pos = ivec3(x, y, z);

                // Density: sphere centered below origin
                let mut density = DensityField::new();
                let offset = pos.as_vec3() * 32.0;
                let center = vec3(16.0, 16.0, 16.0) - offset + vec3(0.0, -5.0, 0.0);
                bevy_sculpter::helpers::fill_sphere(&mut density, center, 25.0);

                // Materials: height-based layers
                let mut mat_field = MaterialField::new();
                mat_field.fill_by_world_height(pos, mesh_size, |world_y| {
                    if world_y > 8.0 { 3 }       // snow
                    else if world_y > 0.0 { 0 }  // grass
                    else if world_y > -5.0 { 2 } // dirt
                    else { 1 }                   // rock
                });

                // Apply steepness-based rock using density sampler closure
                fill_by_steepness(
                    &mut mat_field,
                    |x, y, z| density.get(x, y, z),
                    mat_field.get(16, 16, 16),
                    1,
                    0.6,
                );

                commands.spawn((
                    Chunk,
                    ChunkPos(pos),
                    density,
                    mat_field,
                    DensityFieldDirty,
                    MeshMaterial3d(mat.clone()),
                    Transform::from_translation(pos.as_vec3() * mesh_size),
                ));
            }
        }
    }

    // Lighting
    commands.spawn((
        DirectionalLight { illuminance: 15000.0, shadows_enabled: true, ..default() },
        Transform::from_xyz(5.0, 10.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.insert_resource(AmbientLight { brightness: 400.0, ..default() });

    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(25.0, 20.0, 25.0).looking_at(Vec3::ZERO, Vec3::Y),
        Orbit { dist: 40.0, pitch: 0.5, yaw: 0.785 },
    ));

    // UI
    commands.spawn((
        Text::new("RMB:Add LMB:Remove Shift+Click:Paint 1-4:Material Scroll:Size"),
        Node { position_type: PositionType::Absolute, top: Val::Px(10.0), left: Val::Px(10.0), ..default() },
    ));
}

fn remesh(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mesh_size: Res<DensityFieldMeshSize>,
    chunks: Query<(Entity, &DensityField, &MaterialField, Option<&NeighborDensityFields>), With<DensityFieldDirty>>,
) {
    for (entity, density, materials, neighbors) in chunks.iter() {
        let n = neighbors.cloned().unwrap_or_default();

        let Some(mut mesh) = bevy_sculpter::mesher::generate_mesh_cpu(density, &n, mesh_size.0) else {
            commands.entity(entity).remove::<(Mesh3d, DensityFieldDirty)>();
            continue;
        };

        // The key integration: wrap the mesh with material attributes
        // Pass a closure that samples density - no direct dependency needed
        add_material_attributes(
            &mut mesh,
            |x, y, z| density.get_signed(x, y, z).unwrap_or(1.0),
            materials,
            mesh_size.0,
        );

        commands.entity(entity)
            .insert(Mesh3d(meshes.add(mesh)))
            .remove::<DensityFieldDirty>();
    }
}

fn sculpt(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    window: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform)>,
    time: Res<Time>,
    brush: Res<Brush>,
    mesh_size: Res<DensityFieldMeshSize>,
    mut chunks: Query<(Entity, &ChunkPos, &mut DensityField)>,
) {
    if keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight) {
        return; // Paint-only mode
    }

    let adding = buttons.pressed(MouseButton::Right);
    let removing = buttons.pressed(MouseButton::Left);
    if !adding && !removing {
        return;
    }

    let Some(hit) = raycast(&window, &camera, &chunks, &mesh_size) else { return };
    let rate = if adding { -brush.strength } else { brush.strength };

    for (entity, pos, mut density) in chunks.iter_mut() {
        let local = hit - pos.0.as_vec3() * mesh_size.0;
        let scale = Vec3::splat(32.0) / mesh_size.0;
        let grid_center = local * scale;
        let grid_radius = brush.radius * scale.x;

        if !sphere_aabb(grid_center, grid_radius, Vec3::ZERO, Vec3::splat(32.0)) {
            continue;
        }

        bevy_sculpter::helpers::brush_smooth_timed(
            &mut density, grid_center, grid_radius, rate, time.delta_secs(), 2.0,
        );
        commands.entity(entity).insert(DensityFieldDirty);
    }
}

fn paint(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    window: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform)>,
    brush: Res<Brush>,
    mesh_size: Res<DensityFieldMeshSize>,
    mut chunks: Query<(Entity, &ChunkPos, &DensityField, &mut MaterialField)>,
) {
    if !keys.pressed(KeyCode::ShiftLeft) && !keys.pressed(KeyCode::ShiftRight) {
        return;
    }
    if !buttons.pressed(MouseButton::Left) && !buttons.pressed(MouseButton::Right) {
        return;
    }

    let win = window.single().ok();
    let Some(win) = win else { return };
    let Some(cursor) = win.cursor_position() else { return };
    let Ok((cam, tf)) = camera.single() else { return };
    let Ok(ray) = cam.viewport_to_world(tf, cursor) else { return };

    // Raycast using density fields
    let mut hit_point = None;
    let step = 0.1;
    let mut t = 0.0;
    while t < 100.0 && hit_point.is_none() {
        let pt = ray.origin + ray.direction * t;
        let chunk_coord = (pt / mesh_size.0).floor().as_ivec3();

        for (_, pos, density, _) in chunks.iter() {
            if pos.0 != chunk_coord { continue; }
            let local = pt - pos.0.as_vec3() * mesh_size.0;
            let grid = local * Vec3::splat(32.0) / mesh_size.0;
            if grid.cmpge(Vec3::ZERO).all() && grid.cmplt(Vec3::splat(32.0)).all() {
                if density.get(grid.x as u32, grid.y as u32, grid.z as u32) < 0.0 {
                    hit_point = Some(pt);
                    break;
                }
            }
        }
        t += step;
    }

    let Some(hit) = hit_point else { return };

    for (entity, pos, density, mut materials) in chunks.iter_mut() {
        let local = hit - pos.0.as_vec3() * mesh_size.0;
        let scale = Vec3::splat(32.0) / mesh_size.0;
        let grid_center = local * scale;
        let grid_radius = brush.radius * scale.x;

        if !sphere_aabb(grid_center, grid_radius, Vec3::ZERO, Vec3::splat(32.0)) {
            continue;
        }

        paint_surface(
            &mut materials,
            |x, y, z| density.get(x, y, z),
            grid_center,
            grid_radius,
            brush.material,
            2.0,
        );
        commands.entity(entity).insert(DensityFieldDirty);
    }
}

fn raycast(
    window: &Query<&Window, With<PrimaryWindow>>,
    camera: &Query<(&Camera, &GlobalTransform)>,
    chunks: &Query<(Entity, &ChunkPos, &mut DensityField)>,
    mesh_size: &DensityFieldMeshSize,
) -> Option<Vec3> {
    let win = window.single().ok()?;
    let cursor = win.cursor_position()?;
    let (cam, tf) = camera.single().ok()?;
    let ray = cam.viewport_to_world(tf, cursor).ok()?;

    let step = 0.1;
    let mut t = 0.0;
    while t < 100.0 {
        let pt = ray.origin + ray.direction * t;
        let chunk_coord = (pt / mesh_size.0).floor().as_ivec3();

        for (_, pos, density) in chunks.iter() {
            if pos.0 != chunk_coord { continue; }
            let local = pt - pos.0.as_vec3() * mesh_size.0;
            let grid = local * Vec3::splat(32.0) / mesh_size.0;
            if grid.cmpge(Vec3::ZERO).all() && grid.cmplt(Vec3::splat(32.0)).all() {
                if density.get(grid.x as u32, grid.y as u32, grid.z as u32) < 0.0 {
                    return Some(pt);
                }
            }
        }
        t += step;
    }
    None
}

fn sphere_aabb(center: Vec3, radius: f32, min: Vec3, max: Vec3) -> bool {
    let closest = center.clamp(min, max);
    center.distance_squared(closest) <= radius * radius
}

fn input(
    keys: Res<ButtonInput<KeyCode>>,
    mut scroll: EventReader<MouseWheel>,
    mut brush: ResMut<Brush>,
) {
    for ev in scroll.read() {
        brush.radius = (brush.radius + ev.y * 0.3).clamp(0.5, 8.0);
    }
    if keys.just_pressed(KeyCode::Digit1) { brush.material = 0; }
    if keys.just_pressed(KeyCode::Digit2) { brush.material = 1; }
    if keys.just_pressed(KeyCode::Digit3) { brush.material = 2; }
    if keys.just_pressed(KeyCode::Digit4) { brush.material = 3; }
}

#[derive(Component)]
struct Orbit { dist: f32, pitch: f32, yaw: f32 }

fn orbit_camera(
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    mut q: Query<(&mut Transform, &mut Orbit)>,
) {
    if !buttons.pressed(MouseButton::Middle) { return; }
    let Ok((mut tf, mut orb)) = q.single_mut() else { return };

    orb.yaw -= motion.delta.x * 0.005;
    orb.pitch = (orb.pitch - motion.delta.y * 0.005).clamp(-1.4, 1.4);

    let rot = Quat::from_euler(EulerRot::YXZ, orb.yaw, orb.pitch, 0.0);
    *tf = Transform::from_translation(rot * Vec3::new(0.0, 0.0, orb.dist))
        .looking_at(Vec3::ZERO, Vec3::Y);
}

fn create_textures(images: &mut Assets<Image>) -> Handle<Image> {
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

    let size = 64usize;
    let colors: [[u8; 3]; 4] = [
        [80, 140, 60],   // grass
        [120, 110, 100], // rock
        [140, 100, 60],  // dirt
        [240, 245, 250], // snow
    ];

    let mut data = vec![0u8; size * size * 4 * 4];
    for (layer, color) in colors.iter().enumerate() {
        for y in 0..size {
            for x in 0..size {
                let i = ((layer * size + y) * size + x) * 4;
                let v = (((x / 8 + y / 8) % 2) as i8 * 10 - 5) as i8;
                data[i] = color[0].saturating_add_signed(v);
                data[i + 1] = color[1].saturating_add_signed(v);
                data[i + 2] = color[2].saturating_add_signed(v);
                data[i + 3] = 255;
            }
        }
    }

    images.add(Image::new(
        Extent3d { width: size as u32, height: size as u32, depth_or_array_layers: 4 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        default(),
    ))
}
