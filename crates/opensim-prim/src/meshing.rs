// SPDX-License-Identifier: GPL-3.0-or-later

//! Prim-to-physics-collider meshing pipeline.
//!
//! Converts aurora-prim volume geometry into Rapier 3-D collision shapes
//! (`SharedShape`).  Primitive prims (uncut, unhollow boxes and spheres) get
//! analytic shapes; everything else is meshed through `generate_volume` and
//! turned into a trimesh or convex hull.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

use glam::Vec3;
use rapier3d::geometry::SharedShape;
use rapier3d::math::Vector;
use uuid::Uuid;

use crate::{
    VolumeParams, generate_volume, generate_sculpt, sculpt_placeholder,
    LL_PCODE_PATH_LINE, LL_PCODE_PROFILE_CIRCLE, LL_PCODE_PROFILE_SQUARE,
    LL_SCULPT_FLAG_INVERT, LL_SCULPT_FLAG_MIRROR, LL_SCULPT_TYPE_MASK,
    LL_SCULPT_TYPE_NONE, LL_SCULPT_TYPE_SPHERE, LL_SCULPT_TYPE_TORUS,
    LL_SCULPT_TYPE_PLANE, LL_SCULPT_TYPE_CYLINDER,
};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// How the physics engine should represent a prim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PhysicsShapeType {
    /// Full trimesh (most accurate, most expensive).
    Prim = 0,
    /// No collision shape at all (phantom).
    None = 1,
    /// Convex hull of the prim geometry.
    Convex = 2,
}

/// The result of meshing a single prim for physics.
#[derive(Debug, Clone)]
pub struct MeshingResult {
    /// The collision shape, ready for insertion into a Rapier collider.
    pub shape: SharedShape,
    /// Number of vertices that went into the shape (0 for analytic shapes).
    pub vertex_count: usize,
    /// Number of triangles (0 for analytic / convex shapes).
    pub triangle_count: usize,
    /// Wall-clock time spent generating the shape.
    pub elapsed_us: u64,
}

// ---------------------------------------------------------------------------
// Sculpt types and texture source
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SculptType {
    None,
    Sphere,
    Torus,
    Plane,
    Cylinder,
}

impl SculptType {
    pub fn from_raw(raw: u8) -> (SculptType, bool, bool) {
        let kind = raw & LL_SCULPT_TYPE_MASK;
        let invert = raw & LL_SCULPT_FLAG_INVERT != 0;
        let mirror = raw & LL_SCULPT_FLAG_MIRROR != 0;
        let ty = match kind {
            LL_SCULPT_TYPE_SPHERE => SculptType::Sphere,
            LL_SCULPT_TYPE_TORUS => SculptType::Torus,
            LL_SCULPT_TYPE_PLANE => SculptType::Plane,
            LL_SCULPT_TYPE_CYLINDER => SculptType::Cylinder,
            LL_SCULPT_TYPE_NONE | _ => SculptType::None,
        };
        (ty, invert, mirror)
    }
}

pub struct SculptTextureData {
    pub width: u32,
    pub height: u32,
    pub components: u8,
    pub pixels: Vec<u8>,
}

pub trait SculptTextureSource {
    fn decode_sculpt_texture(&self, texture_id: &Uuid) -> Option<SculptTextureData>;
}

pub struct PlaceholderSculptSource;

impl SculptTextureSource for PlaceholderSculptSource {
    fn decode_sculpt_texture(&self, _texture_id: &Uuid) -> Option<SculptTextureData> {
        None
    }
}

// ---------------------------------------------------------------------------
// Shape cache
// ---------------------------------------------------------------------------

/// Thread-safe cache of previously-computed physics shapes, keyed by a
/// caller-supplied `u64` (e.g. a hash of VolumeParams + size + shape type).
pub struct ShapeCache {
    inner: Mutex<HashMap<u64, SharedShape>>,
}

impl ShapeCache {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }

    /// Look up a cached shape.  Returns `None` on a miss.
    pub fn get(&self, key: u64) -> Option<SharedShape> {
        self.inner.lock().unwrap().get(&key).cloned()
    }

    /// Insert a shape into the cache, returning the previous value (if any).
    pub fn insert(&self, key: u64, shape: SharedShape) -> Option<SharedShape> {
        self.inner.lock().unwrap().insert(key, shape)
    }

    /// Number of cached entries.
    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for ShapeCache {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Analytic shape helpers
// ---------------------------------------------------------------------------

/// Create a box collision shape from half-extents.
pub fn shape_for_box(half_extents: Vec3) -> SharedShape {
    SharedShape::cuboid(half_extents.x, half_extents.y, half_extents.z)
}

/// Create a sphere collision shape.
pub fn shape_for_sphere(radius: f32) -> SharedShape {
    SharedShape::ball(radius)
}

// ---------------------------------------------------------------------------
// Shape-detection helpers
// ---------------------------------------------------------------------------

/// Returns `true` when the volume parameters describe an uncut, unhollow box
/// (square profile, linear path, no twist/taper/shear).
pub fn is_box(params: &VolumeParams) -> bool {
    let p = &params.profile;
    let t = &params.path;

    p.curve_type == LL_PCODE_PROFILE_SQUARE
        && t.curve_type == LL_PCODE_PATH_LINE
        && p.begin == 0.0
        && p.end == 1.0
        && p.hollow == 0.0
        && t.begin == 0.0
        && t.end == 1.0
        && t.twist_begin == 0.0
        && t.twist_end == 0.0
        && t.taper == [0.0, 0.0]
        && t.shear == [0.0, 0.0]
        && t.scale == [1.0, 1.0]
        && params.sculpt.is_none()
}

/// Returns `true` when the volume parameters describe an uncut, unhollow
/// sphere (circle profile, half-circle path, no twist/taper/shear).
pub fn is_sphere(params: &VolumeParams) -> bool {
    let p = &params.profile;
    let t = &params.path;

    p.curve_type == LL_PCODE_PROFILE_CIRCLE
        && t.curve_type == LL_PCODE_PATH_LINE
        && p.begin == 0.0
        && p.end == 1.0
        && p.hollow == 0.0
        && t.begin == 0.0
        && t.end == 1.0
        && t.twist_begin == 0.0
        && t.twist_end == 0.0
        && t.taper == [0.0, 0.0]
        && t.shear == [0.0, 0.0]
        && t.scale == [1.0, 1.0]
        && params.sculpt.is_none()
}

// ---------------------------------------------------------------------------
// Volume -> physics shape
// ---------------------------------------------------------------------------

/// Generate a physics shape from a procedural prim volume.
///
/// Calls `generate_volume` to produce geometry, then converts the resulting
/// faces into a trimesh.  `size` scales the unit-prim positions.  `detail`
/// controls LOD (see `aurora_prim::volume::DETAIL_SCALES`).
pub fn shape_from_volume(
    params: &VolumeParams,
    size: Vec3,
    detail: f32,
) -> MeshingResult {
    let start = Instant::now();
    let mesh = generate_volume(params, detail);

    // Collect all vertices, applying the prim's scale.
    let mut vertices: Vec<Vector> = Vec::new();
    let mut indices: Vec<[u32; 3]> = Vec::new();
    let mut base: u32 = 0;

    for face in &mesh.faces {
        for pos in &face.positions {
            vertices.push(Vector::new(
                pos[0] * size.x,
                pos[1] * size.y,
                pos[2] * size.z,
            ));
        }

        // Convert u16 triangle indices to [u32; 3], offset by base.
        let face_indices = &face.indices;
        let tri_count = face_indices.len() / 3;
        for tri in 0..tri_count {
            let i0 = base + face_indices[tri * 3] as u32;
            let i1 = base + face_indices[tri * 3 + 1] as u32;
            let i2 = base + face_indices[tri * 3 + 2] as u32;
            indices.push([i0, i1, i2]);
        }

        base += face.positions.len() as u32;
    }

    let vertex_count = vertices.len();
    let triangle_count = indices.len();

    // SharedShape::trimesh returns a Result in parry3d 0.31+.
    let shape = if vertices.is_empty() || indices.is_empty() {
        // Degenerate prim — fall back to a tiny sphere.
        SharedShape::ball(0.001)
    } else {
        SharedShape::trimesh(vertices, indices).unwrap_or_else(|_| SharedShape::ball(0.001))
    };

    MeshingResult {
        shape,
        vertex_count,
        triangle_count,
        elapsed_us: start.elapsed().as_micros() as u64,
    }
}

/// Generate a convex hull physics shape from a procedural prim volume.
fn shape_convex_from_volume(
    params: &VolumeParams,
    size: Vec3,
    detail: f32,
) -> MeshingResult {
    let start = Instant::now();
    let mesh = generate_volume(params, detail);

    let mut vertices: Vec<Vector> = Vec::new();
    for face in &mesh.faces {
        for pos in &face.positions {
            vertices.push(Vector::new(
                pos[0] * size.x,
                pos[1] * size.y,
                pos[2] * size.z,
            ));
        }
    }

    let vertex_count = vertices.len();

    let shape = if vertices.len() < 4 {
        SharedShape::ball(0.001)
    } else {
        SharedShape::convex_hull(&vertices).unwrap_or_else(|| SharedShape::ball(0.001))
    };

    MeshingResult {
        shape,
        vertex_count,
        triangle_count: 0,
        elapsed_us: start.elapsed().as_micros() as u64,
    }
}

// ---------------------------------------------------------------------------
// Sculpt -> physics shape
// ---------------------------------------------------------------------------

pub fn shape_from_sculpt(
    params: &VolumeParams,
    size: Vec3,
    detail: f32,
    texture_data: Option<&SculptTextureData>,
) -> MeshingResult {
    let start = Instant::now();

    let mesh = match texture_data {
        Some(data) => generate_sculpt(
            params,
            detail,
            data.width,
            data.height,
            data.components,
            &data.pixels,
        ),
        None => sculpt_placeholder(params, detail),
    };

    let mut vertices: Vec<Vector> = Vec::new();
    let mut indices: Vec<[u32; 3]> = Vec::new();
    let mut base: u32 = 0;

    for face in &mesh.faces {
        for pos in &face.positions {
            vertices.push(Vector::new(
                pos[0] * size.x,
                pos[1] * size.y,
                pos[2] * size.z,
            ));
        }

        let face_indices = &face.indices;
        let tri_count = face_indices.len() / 3;
        for tri in 0..tri_count {
            let i0 = base + face_indices[tri * 3] as u32;
            let i1 = base + face_indices[tri * 3 + 1] as u32;
            let i2 = base + face_indices[tri * 3 + 2] as u32;
            indices.push([i0, i1, i2]);
        }

        base += face.positions.len() as u32;
    }

    let vertex_count = vertices.len();
    let triangle_count = indices.len();

    let shape = if vertices.is_empty() || indices.is_empty() {
        SharedShape::ball(0.001)
    } else {
        SharedShape::trimesh(vertices, indices).unwrap_or_else(|_| SharedShape::ball(0.001))
    };

    MeshingResult {
        shape,
        vertex_count,
        triangle_count,
        elapsed_us: start.elapsed().as_micros() as u64,
    }
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

/// Create a physics collision shape for a prim.
///
/// 1. If `shape_type` is `None`, returns `None` (phantom prim).
/// 2. Checks `cache` for a previously-computed shape (keyed by `cache_key`).
/// 3. For default-param boxes/spheres, returns an analytic shape.
/// 4. Otherwise generates a trimesh or convex hull from volume geometry.
///
/// The caller supplies a `cache_key` that uniquely identifies the combination
/// of `params + size + shape_type` (e.g. via hashing).  Pass `0` to bypass
/// the cache.
pub fn create_physics_shape(
    params: &VolumeParams,
    size: Vec3,
    shape_type: PhysicsShapeType,
    cache: &ShapeCache,
    cache_key: u64,
) -> Option<MeshingResult> {
    create_physics_shape_with_sculpt(params, size, shape_type, cache, cache_key, None)
}

pub fn create_physics_shape_with_sculpt(
    params: &VolumeParams,
    size: Vec3,
    shape_type: PhysicsShapeType,
    cache: &ShapeCache,
    cache_key: u64,
    sculpt_source: Option<&dyn SculptTextureSource>,
) -> Option<MeshingResult> {
    if shape_type == PhysicsShapeType::None {
        return None;
    }

    // Cache lookup (skip when key is 0).
    if cache_key != 0 {
        if let Some(shape) = cache.get(cache_key) {
            return Some(MeshingResult {
                shape,
                vertex_count: 0,
                triangle_count: 0,
                elapsed_us: 0,
            });
        }
    }

    let start = Instant::now();

    // Sculpted prims bypass analytic shape detection and volume meshing.
    if params.is_sculpt() {
        let detail = PhysicsLod::from_size(size).detail();
        let texture_data = params.sculpt.and_then(|sp| {
            sculpt_source.and_then(|src| src.decode_sculpt_texture(&sp.texture))
        });
        let result = shape_from_sculpt(params, size, detail, texture_data.as_ref());
        if cache_key != 0 {
            cache.insert(cache_key, result.shape.clone());
        }
        return Some(result);
    }

    // Try analytic shapes first.
    if is_box(params) {
        let half = size * 0.5;
        let shape = shape_for_box(half);
        let result = MeshingResult {
            shape: shape.clone(),
            vertex_count: 0,
            triangle_count: 0,
            elapsed_us: start.elapsed().as_micros() as u64,
        };
        if cache_key != 0 {
            cache.insert(cache_key, shape);
        }
        return Some(result);
    }

    if is_sphere(params) {
        // OpenSim sphere prims are axis-aligned ellipsoids only when all three
        // scale components are equal; otherwise we must mesh.  Use the
        // minimum extent as the radius for a conservative bounding sphere.
        let r = size.x.min(size.y).min(size.z) * 0.5;
        if (size.x - size.y).abs() < 1e-4 && (size.y - size.z).abs() < 1e-4 {
            let shape = shape_for_sphere(r);
            let result = MeshingResult {
                shape: shape.clone(),
                vertex_count: 0,
                triangle_count: 0,
                elapsed_us: start.elapsed().as_micros() as u64,
            };
            if cache_key != 0 {
                cache.insert(cache_key, shape);
            }
            return Some(result);
        }
    }

    // Fall through to mesh generation.
    let detail = PhysicsLod::from_size(size).detail();
    let result = match shape_type {
        PhysicsShapeType::Convex => shape_convex_from_volume(params, size, detail),
        PhysicsShapeType::Prim => shape_from_volume(params, size, detail),
        PhysicsShapeType::None => unreachable!(),
    };

    if cache_key != 0 {
        cache.insert(cache_key, result.shape.clone());
    }

    Some(result)
}

// ---------------------------------------------------------------------------
// LOD selection
// ---------------------------------------------------------------------------

/// LOD detail levels for physics meshing, matching C# Meshmerizer thresholds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PhysicsLod {
    Lowest,
    Low,
    Medium,
    High,
}

impl PhysicsLod {
    /// Select LOD based on the prim's maximum scale dimension.
    pub fn from_size(size: Vec3) -> Self {
        let max_dim = size.x.max(size.y).max(size.z);
        if max_dim < 4.0 {
            PhysicsLod::Lowest
        } else if max_dim < 16.0 {
            PhysicsLod::Low
        } else if max_dim < 64.0 {
            PhysicsLod::Medium
        } else {
            PhysicsLod::High
        }
    }

    /// The detail parameter to pass to `generate_volume`.
    pub fn detail(&self) -> f32 {
        match self {
            PhysicsLod::Lowest => 0.25,
            PhysicsLod::Low => 0.5,
            PhysicsLod::Medium => 1.0,
            PhysicsLod::High => 2.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PathParams, ProfileParams, SculptParams};

    fn box_params() -> VolumeParams {
        VolumeParams {
            profile: ProfileParams {
                curve_type: LL_PCODE_PROFILE_SQUARE,
                begin: 0.0,
                end: 1.0,
                hollow: 0.0,
            },
            path: PathParams::default(),
            sculpt: None,
        }
    }

    fn sphere_params() -> VolumeParams {
        VolumeParams {
            profile: ProfileParams {
                curve_type: LL_PCODE_PROFILE_CIRCLE,
                begin: 0.0,
                end: 1.0,
                hollow: 0.0,
            },
            path: PathParams::default(),
            sculpt: None,
        }
    }

    fn sculpt_params(sculpt_type: u8) -> VolumeParams {
        VolumeParams {
            profile: ProfileParams::default(),
            path: PathParams::default(),
            sculpt: Some(SculptParams {
                texture: Uuid::from_u128(0xdead_beef),
                sculpt_type,
            }),
        }
    }

    #[test]
    fn analytic_box_shape() {
        let params = box_params();
        let size = Vec3::new(2.0, 4.0, 6.0);
        let cache = ShapeCache::new();

        let result = create_physics_shape(
            &params,
            size,
            PhysicsShapeType::Prim,
            &cache,
            0,
        );

        let result = result.expect("box should produce a shape");
        // Analytic shapes have zero vertex/triangle counts.
        assert_eq!(result.vertex_count, 0);
        assert_eq!(result.triangle_count, 0);
    }

    #[test]
    fn analytic_sphere_shape() {
        let params = sphere_params();
        let size = Vec3::splat(3.0);
        let cache = ShapeCache::new();

        let result = create_physics_shape(
            &params,
            size,
            PhysicsShapeType::Prim,
            &cache,
            0,
        );

        let result = result.expect("sphere should produce a shape");
        assert_eq!(result.vertex_count, 0);
        assert_eq!(result.triangle_count, 0);
    }

    #[test]
    fn phantom_returns_none() {
        let params = box_params();
        let size = Vec3::splat(1.0);
        let cache = ShapeCache::new();

        let result = create_physics_shape(
            &params,
            size,
            PhysicsShapeType::None,
            &cache,
            0,
        );

        assert!(result.is_none());
    }

    #[test]
    fn cache_hit_and_miss() {
        let params = box_params();
        let size = Vec3::splat(1.0);
        let cache = ShapeCache::new();
        let key = 42;

        assert!(cache.is_empty());

        // First call: cache miss, populates the cache.
        let r1 = create_physics_shape(
            &params,
            size,
            PhysicsShapeType::Prim,
            &cache,
            key,
        );
        assert!(r1.is_some());
        assert_eq!(cache.len(), 1);

        // Second call: cache hit.
        let r2 = create_physics_shape(
            &params,
            size,
            PhysicsShapeType::Prim,
            &cache,
            key,
        );
        let r2 = r2.unwrap();
        // Cache hits report zero elapsed time.
        assert_eq!(r2.elapsed_us, 0);
    }

    #[test]
    fn meshed_trimesh_produces_geometry() {
        // A hollowed box cannot use the analytic path, so it gets meshed.
        let params = VolumeParams {
            profile: ProfileParams {
                curve_type: LL_PCODE_PROFILE_SQUARE,
                begin: 0.0,
                end: 1.0,
                hollow: 0.5, // hollow forces meshing
            },
            path: PathParams::default(),
            sculpt: None,
        };
        let size = Vec3::splat(1.0);
        let cache = ShapeCache::new();

        let result = create_physics_shape(
            &params,
            size,
            PhysicsShapeType::Prim,
            &cache,
            0,
        );

        let result = result.expect("hollowed box should produce a shape");
        assert!(result.vertex_count > 0, "trimesh should have vertices");
        assert!(result.triangle_count > 0, "trimesh should have triangles");
    }

    #[test]
    fn convex_hull_from_volume() {
        // Use a cylinder-ish shape (circle profile, linear path) to exercise
        // convex hull generation.
        let params = sphere_params();
        let size = Vec3::new(1.0, 2.0, 1.0); // non-uniform => not analytic sphere
        let cache = ShapeCache::new();

        let result = create_physics_shape(
            &params,
            size,
            PhysicsShapeType::Convex,
            &cache,
            0,
        );

        let result = result.expect("convex hull should produce a shape");
        assert!(result.vertex_count > 0, "convex hull should have input vertices");
    }

    #[test]
    fn is_box_detection() {
        assert!(is_box(&box_params()));
        // Hollowed box is not an analytic box.
        let mut p = box_params();
        p.profile.hollow = 0.25;
        assert!(!is_box(&p));
    }

    #[test]
    fn is_sphere_detection() {
        assert!(is_sphere(&sphere_params()));
        // Cut sphere is not analytic.
        let mut p = sphere_params();
        p.profile.begin = 0.1;
        assert!(!is_sphere(&p));
    }

    #[test]
    fn lod_from_size_tiny() {
        assert_eq!(PhysicsLod::from_size(Vec3::splat(1.0)), PhysicsLod::Lowest);
        assert_eq!(PhysicsLod::from_size(Vec3::splat(3.9)), PhysicsLod::Lowest);
    }

    #[test]
    fn lod_from_size_medium() {
        assert_eq!(PhysicsLod::from_size(Vec3::splat(4.0)), PhysicsLod::Low);
        assert_eq!(PhysicsLod::from_size(Vec3::splat(10.0)), PhysicsLod::Low);
        assert_eq!(PhysicsLod::from_size(Vec3::splat(16.0)), PhysicsLod::Medium);
        assert_eq!(PhysicsLod::from_size(Vec3::splat(50.0)), PhysicsLod::Medium);
    }

    #[test]
    fn lod_from_size_large() {
        assert_eq!(PhysicsLod::from_size(Vec3::splat(64.0)), PhysicsLod::High);
        assert_eq!(PhysicsLod::from_size(Vec3::splat(200.0)), PhysicsLod::High);
    }

    #[test]
    fn lod_uses_max_dimension() {
        // Non-uniform: largest dimension should determine LOD
        assert_eq!(PhysicsLod::from_size(Vec3::new(1.0, 1.0, 100.0)), PhysicsLod::High);
        assert_eq!(PhysicsLod::from_size(Vec3::new(100.0, 1.0, 1.0)), PhysicsLod::High);
    }

    #[test]
    fn lod_detail_values() {
        assert_eq!(PhysicsLod::Lowest.detail(), 0.25);
        assert_eq!(PhysicsLod::Low.detail(), 0.5);
        assert_eq!(PhysicsLod::Medium.detail(), 1.0);
        assert_eq!(PhysicsLod::High.detail(), 2.0);
    }

    // --- Sculpt tests ---

    #[test]
    fn test_sculpt_type_from_raw() {
        assert_eq!(SculptType::from_raw(LL_SCULPT_TYPE_NONE), (SculptType::None, false, false));
        assert_eq!(SculptType::from_raw(LL_SCULPT_TYPE_SPHERE), (SculptType::Sphere, false, false));
        assert_eq!(SculptType::from_raw(LL_SCULPT_TYPE_TORUS), (SculptType::Torus, false, false));
        assert_eq!(SculptType::from_raw(LL_SCULPT_TYPE_PLANE), (SculptType::Plane, false, false));
        assert_eq!(SculptType::from_raw(LL_SCULPT_TYPE_CYLINDER), (SculptType::Cylinder, false, false));
        assert_eq!(SculptType::from_raw(7), (SculptType::None, false, false));
    }

    #[test]
    fn test_sculpt_type_flags() {
        let (ty, inv, mir) = SculptType::from_raw(LL_SCULPT_TYPE_SPHERE | LL_SCULPT_FLAG_INVERT);
        assert_eq!(ty, SculptType::Sphere);
        assert!(inv);
        assert!(!mir);

        let (ty, inv, mir) = SculptType::from_raw(LL_SCULPT_TYPE_TORUS | LL_SCULPT_FLAG_MIRROR);
        assert_eq!(ty, SculptType::Torus);
        assert!(!inv);
        assert!(mir);

        let (ty, inv, mir) = SculptType::from_raw(
            LL_SCULPT_TYPE_CYLINDER | LL_SCULPT_FLAG_INVERT | LL_SCULPT_FLAG_MIRROR,
        );
        assert_eq!(ty, SculptType::Cylinder);
        assert!(inv);
        assert!(mir);

        let (ty, inv, mir) = SculptType::from_raw(LL_SCULPT_TYPE_PLANE);
        assert_eq!(ty, SculptType::Plane);
        assert!(!inv);
        assert!(!mir);
    }

    #[test]
    fn test_sculpt_placeholder_produces_shape() {
        let params = sculpt_params(LL_SCULPT_TYPE_SPHERE);
        let size = Vec3::splat(1.0);
        let result = shape_from_sculpt(&params, size, 1.0, None);
        assert!(result.vertex_count > 0);
        assert!(result.triangle_count > 0);
    }

    #[test]
    fn test_sculpt_with_fake_texture() {
        let params = sculpt_params(LL_SCULPT_TYPE_SPHERE);
        let size = Vec3::splat(1.0);

        let mut pixels = vec![0u8; 4 * 4 * 3];
        for y in 0..4u32 {
            for x in 0..4u32 {
                let i = (y * 4 + x) as usize * 3;
                pixels[i] = (x * 85) as u8;
                pixels[i + 1] = (y * 85) as u8;
                pixels[i + 2] = 128;
            }
        }

        let tex = SculptTextureData {
            width: 4,
            height: 4,
            components: 3,
            pixels,
        };

        let result = shape_from_sculpt(&params, size, 1.0, Some(&tex));
        assert!(result.vertex_count > 0);
        assert!(result.triangle_count > 0);
    }

    #[test]
    fn test_placeholder_sculpt_source() {
        let src = PlaceholderSculptSource;
        assert!(src.decode_sculpt_texture(&Uuid::nil()).is_none());
        assert!(src.decode_sculpt_texture(&Uuid::from_u128(42)).is_none());
    }
}
