//! Material painting brushes.

use bevy::prelude::*;

use crate::DENSITY_FIELD_SIZE;
use super::MaterialField;

/// Paints a sphere of material with hard edges.
///
/// All voxels within the radius are set to the specified material ID.
///
/// # Arguments
/// * `field` - Material field to modify
/// * `center` - Brush center in grid coordinates (0-32)
/// * `radius` - Brush radius in grid units
/// * `material_id` - Material to paint
///
/// # Example
/// ```
/// use bevy::prelude::*;
/// use bevy_sculpter::material_field::{MaterialField, paint_sphere};
///
/// let mut field = MaterialField::new();
/// paint_sphere(&mut field, vec3(16.0, 16.0, 16.0), 5.0, 2);
/// ```
pub fn paint_sphere(field: &mut MaterialField, center: Vec3, radius: f32, material_id: u8) {
    let min = (center - Vec3::splat(radius + 1.0))
        .max(Vec3::ZERO)
        .as_ivec3();
    let max = (center + Vec3::splat(radius + 1.0))
        .min(DENSITY_FIELD_SIZE.as_vec3() - Vec3::ONE)
        .as_ivec3();

    let radius_sq = radius * radius;

    for z in min.z..=max.z {
        for y in min.y..=max.y {
            for x in min.x..=max.x {
                let pos = vec3(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5);
                let dist_sq = pos.distance_squared(center);

                if dist_sq <= radius_sq {
                    field.set(x as u32, y as u32, z as u32, material_id);
                }
            }
        }
    }
}

/// Paints material with probability falloff from center.
///
/// Voxels near the center are more likely to be painted than those at the edge,
/// creating a softer, more natural painted appearance.
///
/// # Arguments
/// * `field` - Material field to modify
/// * `center` - Brush center in grid coordinates
/// * `radius` - Brush radius in grid units
/// * `material_id` - Material to paint
/// * `strength` - Probability at center (0.0-1.0)
/// * `falloff` - Falloff curve power (1.0 = linear, 2.0 = quadratic)
/// * `rng` - Random number generator function returning 0.0-1.0
///
/// # Example
/// ```ignore
/// use bevy::prelude::*;
/// use bevy_sculpter::material_field::{MaterialField, paint_sphere_smooth};
/// use rand::Rng;
///
/// let mut field = MaterialField::new();
/// let mut rng = rand::thread_rng();
/// paint_sphere_smooth(
///     &mut field,
///     vec3(16.0, 16.0, 16.0),
///     5.0,
///     2,
///     0.8,
///     2.0,
///     || rng.gen::<f32>(),
/// );
/// ```
pub fn paint_sphere_smooth<R>(
    field: &mut MaterialField,
    center: Vec3,
    radius: f32,
    material_id: u8,
    strength: f32,
    falloff: f32,
    mut rng: R,
) where
    R: FnMut() -> f32,
{
    let min = (center - Vec3::splat(radius + 1.0))
        .max(Vec3::ZERO)
        .as_ivec3();
    let max = (center + Vec3::splat(radius + 1.0))
        .min(DENSITY_FIELD_SIZE.as_vec3() - Vec3::ONE)
        .as_ivec3();

    for z in min.z..=max.z {
        for y in min.y..=max.y {
            for x in min.x..=max.x {
                let pos = vec3(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5);
                let dist = pos.distance(center);

                if dist > radius {
                    continue;
                }

                // Calculate paint probability
                let t = 1.0 - (dist / radius);
                let probability = strength * t.powf(falloff);

                if rng() < probability {
                    field.set(x as u32, y as u32, z as u32, material_id);
                }
            }
        }
    }
}

/// Paints material only on voxels near the surface.
///
/// Uses density values to only paint voxels close to the isosurface,
/// useful for "spray painting" visible surfaces without affecting interior.
///
/// # Arguments
/// * `material_field` - Material field to modify
/// * `density_field` - Density field for surface detection
/// * `center` - Brush center in grid coordinates
/// * `radius` - Brush radius in grid units
/// * `material_id` - Material to paint
/// * `surface_threshold` - Max absolute density to consider "near surface"
pub fn paint_surface(
    material_field: &mut MaterialField,
    density_field: &crate::density_field::DensityField,
    center: Vec3,
    radius: f32,
    material_id: u8,
    surface_threshold: f32,
) {
    let min = (center - Vec3::splat(radius + 1.0))
        .max(Vec3::ZERO)
        .as_ivec3();
    let max = (center + Vec3::splat(radius + 1.0))
        .min(DENSITY_FIELD_SIZE.as_vec3() - Vec3::ONE)
        .as_ivec3();

    let radius_sq = radius * radius;

    for z in min.z..=max.z {
        for y in min.y..=max.y {
            for x in min.x..=max.x {
                let pos = vec3(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5);
                let dist_sq = pos.distance_squared(center);

                if dist_sq > radius_sq {
                    continue;
                }

                // Only paint voxels near the surface
                let density = density_field.get(x as u32, y as u32, z as u32);
                if density.abs() <= surface_threshold {
                    material_field.set(x as u32, y as u32, z as u32, material_id);
                }
            }
        }
    }
}

/// Fills materials based on height within chunk.
///
/// Convenience function for layered terrain materials.
///
/// # Arguments
/// * `field` - Material field to modify
/// * `layers` - Slice of (max_height, material_id) pairs, sorted by height ascending
///
/// # Example
/// ```
/// use bevy_sculpter::material_field::{MaterialField, brushes::fill_height_layers};
///
/// let mut field = MaterialField::new();
/// fill_height_layers(&mut field, &[
///     (8.0, 0),   // Stone below y=8
///     (20.0, 1),  // Dirt from y=8 to y=20
///     (28.0, 2),  // Grass from y=20 to y=28
///     (32.0, 3),  // Snow above y=28
/// ]);
/// ```
pub fn fill_height_layers(field: &mut MaterialField, layers: &[(f32, u8)]) {
    for z in 0..DENSITY_FIELD_SIZE.z {
        for y in 0..DENSITY_FIELD_SIZE.y {
            let height = y as f32;
            let material = layers
                .iter()
                .find(|(max_h, _)| height < *max_h)
                .map(|(_, mat)| *mat)
                .unwrap_or(layers.last().map(|(_, m)| *m).unwrap_or(0));

            for x in 0..DENSITY_FIELD_SIZE.x {
                field.set(x, y, z, material);
            }
        }
    }
}

/// Fills materials based on slope/steepness using density gradients.
///
/// Useful for placing rock on steep surfaces, grass on flat areas.
///
/// # Arguments
/// * `material_field` - Material field to modify
/// * `density_field` - Density field for gradient calculation
/// * `flat_material` - Material for flat surfaces (normal pointing up)
/// * `steep_material` - Material for steep surfaces
/// * `steepness_threshold` - Steepness value (0-1) above which steep_material is used
pub fn fill_by_steepness(
    material_field: &mut MaterialField,
    density_field: &crate::density_field::DensityField,
    flat_material: u8,
    steep_material: u8,
    steepness_threshold: f32,
) {
    for z in 1..(DENSITY_FIELD_SIZE.z - 1) {
        for y in 1..(DENSITY_FIELD_SIZE.y - 1) {
            for x in 1..(DENSITY_FIELD_SIZE.x - 1) {
                // Compute gradient via central differences
                let dx = density_field.get(x + 1, y, z) - density_field.get(x - 1, y, z);
                let dy = density_field.get(x, y + 1, z) - density_field.get(x, y - 1, z);
                let dz = density_field.get(x, y, z + 1) - density_field.get(x, y, z - 1);

                let gradient = Vec3::new(dx, dy, dz);
                let len = gradient.length();

                if len < 0.001 {
                    material_field.set(x, y, z, flat_material);
                    continue;
                }

                let normal = gradient / len;
                let steepness = 1.0 - normal.y.abs();

                let material = if steepness > steepness_threshold {
                    steep_material
                } else {
                    flat_material
                };

                material_field.set(x, y, z, material);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paint_sphere() {
        let mut field = MaterialField::new();
        paint_sphere(&mut field, vec3(16.0, 16.0, 16.0), 3.0, 5);

        // Center should be painted
        assert_eq!(field.get(16, 16, 16), 5);

        // Corner should be unpainted
        assert_eq!(field.get(0, 0, 0), 0);
    }

    #[test]
    fn test_fill_height_layers() {
        let mut field = MaterialField::new();
        fill_height_layers(&mut field, &[(10.0, 1), (20.0, 2), (32.0, 3)]);

        assert_eq!(field.get(0, 5, 0), 1);  // Below 10
        assert_eq!(field.get(0, 15, 0), 2); // Between 10-20
        assert_eq!(field.get(0, 25, 0), 3); // Above 20
    }
}
