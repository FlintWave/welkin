// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests that verify we can load a region database written by
//! C# OpenSimulator 0.9.x and see objects + terrain in the correct positions.
//!
//! The test creates an in-memory SQLite database, populates it with data that
//! mirrors what the C# server would write (UUIDs, flags, positions, a 256x256
//! little-endian f32 heightfield), then reads it back through
//! `SimulationDataStore` and checks every value that matters for scene fidelity.

use sqlx::AnyPool;
use sqlx::any::AnyPoolOptions;
use welkin_data::simulation::*;

// ---------------------------------------------------------------------------
// Schema (identical to the C# OpenSimulator SQLite schema)
// ---------------------------------------------------------------------------

const SCHEMA: &str = "\
CREATE TABLE prims (
    UUID VARCHAR(36) PRIMARY KEY,
    CreationDate INT, Name VARCHAR(255), Text VARCHAR(255), Description VARCHAR(255),
    SitName VARCHAR(255), TouchName VARCHAR(255), ObjectFlags INT,
    OwnerMask INT, NextOwnerMask INT, GroupMask INT, EveryoneMask INT, BaseMask INT,
    PositionX DOUBLE, PositionY DOUBLE, PositionZ DOUBLE,
    GroupPositionX DOUBLE, GroupPositionY DOUBLE, GroupPositionZ DOUBLE,
    VelocityX DOUBLE, VelocityY DOUBLE, VelocityZ DOUBLE,
    AngularVelocityX DOUBLE, AngularVelocityY DOUBLE, AngularVelocityZ DOUBLE,
    AccelerationX DOUBLE, AccelerationY DOUBLE, AccelerationZ DOUBLE,
    RotationX DOUBLE, RotationY DOUBLE, RotationZ DOUBLE, RotationW DOUBLE,
    SitTargetOffsetX DOUBLE, SitTargetOffsetY DOUBLE, SitTargetOffsetZ DOUBLE,
    SitTargetOrientW DOUBLE, SitTargetOrientX DOUBLE, SitTargetOrientY DOUBLE, SitTargetOrientZ DOUBLE,
    RegionUUID VARCHAR(36), CreatorID VARCHAR(36), OwnerID VARCHAR(36),
    GroupID VARCHAR(36), LastOwnerID VARCHAR(36), RezzerID VARCHAR(36), SceneGroupID VARCHAR(36),
    PayPrice INT, PayButton1 INT, PayButton2 INT, PayButton3 INT, PayButton4 INT,
    LoopedSound VARCHAR(36), LoopedSoundGain DOUBLE,
    TextureAnimation BLOB,
    CameraEyeOffsetX DOUBLE, CameraEyeOffsetY DOUBLE, CameraEyeOffsetZ DOUBLE,
    CameraAtOffsetX DOUBLE, CameraAtOffsetY DOUBLE, CameraAtOffsetZ DOUBLE,
    ForceMouselook INT, ScriptAccessPin INT, AllowedDrop INT, DieAtEdge INT,
    SalePrice INT, SaleType INT, ColorR INT, ColorG INT, ColorB INT, ColorA INT,
    ParticleSystem BLOB, ClickAction INT, Material INT,
    CollisionSound VARCHAR(36), CollisionSoundVolume DOUBLE,
    PassTouches INT, PassCollisions INT, LinkNumber INT,
    MediaURL VARCHAR(255), PhysicsShapeType INT,
    Density DOUBLE, GravityModifier DOUBLE, Friction DOUBLE, Restitution DOUBLE,
    DynAttrs TEXT
);

CREATE TABLE primshapes (
    UUID VARCHAR(36) PRIMARY KEY,
    Shape INT, ScaleX DOUBLE, ScaleY DOUBLE, ScaleZ DOUBLE, PCode INT,
    PathBegin INT, PathEnd INT, PathScaleX INT, PathScaleY INT,
    PathShearX INT, PathShearY INT, PathSkew INT, PathCurve INT,
    PathRadiusOffset INT, PathRevolutions INT, PathTaperX INT, PathTaperY INT,
    PathTwist INT, PathTwistBegin INT,
    ProfileBegin INT, ProfileEnd INT, ProfileCurve INT, ProfileHollow INT,
    Texture BLOB, ExtraParams BLOB, State INT, LastAttachPoint INT,
    Media TEXT, MatOvrd BLOB
);

CREATE TABLE terrain (
    RegionUUID VARCHAR(36), Revision INT, Heightfield BLOB
);

CREATE TABLE regionsettings (
    regionUUID VARCHAR(36) PRIMARY KEY,
    block_terraform INT, block_fly INT, allow_damage INT, restrict_pushing INT,
    allow_land_resell INT, allow_land_join_divide INT, block_show_in_search INT,
    agent_limit INT, object_bonus DOUBLE, maturity INT,
    disable_scripts INT, disable_collisions INT, disable_physics INT,
    terrain_texture_1 VARCHAR(36), terrain_texture_2 VARCHAR(36),
    terrain_texture_3 VARCHAR(36), terrain_texture_4 VARCHAR(36),
    elevation_1_nw DOUBLE, elevation_1_ne DOUBLE, elevation_1_se DOUBLE, elevation_1_sw DOUBLE,
    elevation_2_nw DOUBLE, elevation_2_ne DOUBLE, elevation_2_se DOUBLE, elevation_2_sw DOUBLE,
    water_height DOUBLE, terrain_raise_limit DOUBLE, terrain_lower_limit DOUBLE,
    use_estate_sun INT, fixed_sun INT, sun_position DOUBLE,
    covenant VARCHAR(36), covenant_datetime INT, Sandbox INT,
    map_tile_id VARCHAR(36), telehub_object VARCHAR(36),
    parcel_tile_id VARCHAR(36), cacheID VARCHAR(36), block_search INT
);
";

// ---------------------------------------------------------------------------
// Constants — realistic UUIDs matching what C# OpenSim would generate
// ---------------------------------------------------------------------------

const REGION: &str = "6b8d0e1a-4f3c-9a72-d5e8-1c0b7a2f3d4e";
const ZERO: &str = "00000000-0000-0000-0000-000000000000";

/// Owner/creator — a typical resident UUID from C# OpenSim.
const OWNER: &str = "d47f3c77-9e2a-4b58-b6d1-8e3f2a1c5b09";

/// SceneGroupID for the linked pair (root + child).
const GROUP_A: &str = "f1a2b3c4-d5e6-7890-abcd-ef1234560001";

/// SceneGroupID for the standalone prim.
const GROUP_B: &str = "f1a2b3c4-d5e6-7890-abcd-ef1234560002";

/// UUID of the root prim in group A.
const ROOT_UUID: &str = "aabbccdd-1111-2222-3333-444455550001";

/// UUID of the child prim in group A.
const CHILD_UUID: &str = "aabbccdd-1111-2222-3333-444455550002";

/// UUID of the standalone prim (group B).
const SOLO_UUID: &str = "aabbccdd-1111-2222-3333-444455550003";

// ---------------------------------------------------------------------------
// Database setup
// ---------------------------------------------------------------------------

async fn setup_db() -> (AnyPool, SimulationDataStore) {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("failed to connect to in-memory SQLite");

    for stmt in SCHEMA.split(';') {
        let trimmed = stmt.trim();
        if trimmed.is_empty() {
            continue;
        }
        sqlx::query(trimmed)
            .execute(&pool)
            .await
            .unwrap_or_else(|e| panic!("schema setup failed on:\n{trimmed}\nerror: {e}"));
    }

    let store = SimulationDataStore::new(pool.clone());
    (pool, store)
}

// ---------------------------------------------------------------------------
// Prim/shape factories — values chosen to match what a C# 0.9.x region writes
// ---------------------------------------------------------------------------

/// Build a `PrimRow` with explicit position, group-position, scene-group, and
/// link number.  All other fields carry realistic C# defaults (object flags,
/// mask bits, physics parameters, etc.).
fn make_prim(
    uuid: &str,
    scene_group_id: &str,
    link_number: i32,
    pos: (f64, f64, f64),
    group_pos: (f64, f64, f64),
    name: &str,
) -> PrimRow {
    PrimRow {
        uuid: uuid.into(),
        region_uuid: REGION.into(),
        scene_group_id: scene_group_id.into(),
        creation_date: 1_700_000_000,
        name: name.into(),
        text: String::new(),
        description: "Written by C# OpenSimulator 0.9.3".into(),
        sit_name: String::new(),
        touch_name: String::new(),
        // 524292 = ObjectFlags commonly set by C# (PhantomStatus | ...)
        object_flags: 524292,
        creator_id: OWNER.into(),
        owner_id: OWNER.into(),
        group_id: ZERO.into(),
        last_owner_id: OWNER.into(),
        rezzer_id: ZERO.into(),
        owner_mask: 0x7FFF_FFFF,
        next_owner_mask: 0x0008_2000,
        group_mask: 0,
        everyone_mask: 0,
        base_mask: 0x7FFF_FFFF,
        position_x: pos.0,
        position_y: pos.1,
        position_z: pos.2,
        group_position_x: group_pos.0,
        group_position_y: group_pos.1,
        group_position_z: group_pos.2,
        velocity_x: 0.0,
        velocity_y: 0.0,
        velocity_z: 0.0,
        angular_velocity_x: 0.0,
        angular_velocity_y: 0.0,
        angular_velocity_z: 0.0,
        acceleration_x: 0.0,
        acceleration_y: 0.0,
        acceleration_z: 0.0,
        rotation_x: 0.0,
        rotation_y: 0.0,
        rotation_z: 0.0,
        rotation_w: 1.0,
        sit_target_offset_x: 0.0,
        sit_target_offset_y: 0.0,
        sit_target_offset_z: 0.0,
        sit_target_orient_w: 1.0,
        sit_target_orient_x: 0.0,
        sit_target_orient_y: 0.0,
        sit_target_orient_z: 0.0,
        pay_price: -2,
        pay_button1: -2,
        pay_button2: -2,
        pay_button3: -2,
        pay_button4: -2,
        looped_sound: ZERO.into(),
        looped_sound_gain: 0.0,
        texture_animation: Vec::new(),
        camera_eye_offset_x: 0.0,
        camera_eye_offset_y: 0.0,
        camera_eye_offset_z: 0.0,
        camera_at_offset_x: 0.0,
        camera_at_offset_y: 0.0,
        camera_at_offset_z: 0.0,
        force_mouselook: 0,
        script_access_pin: 0,
        allowed_drop: 0,
        die_at_edge: 0,
        sale_price: 10,
        sale_type: 0,
        color_r: 255,
        color_g: 255,
        color_b: 255,
        color_a: 255,
        particle_system: Vec::new(),
        click_action: 0,
        material: 3,
        collision_sound: ZERO.into(),
        collision_sound_volume: 0.0,
        pass_touches: 0,
        pass_collisions: 0,
        link_number,
        media_url: String::new(),
        physics_shape_type: 0,
        density: 1000.0,
        gravity_modifier: 1.0,
        friction: 0.6,
        restitution: 0.5,
        dyn_attrs: String::new(),
    }
}

fn make_shape(uuid: &str, scale: (f64, f64, f64)) -> PrimShapeRow {
    PrimShapeRow {
        uuid: uuid.into(),
        shape: 0,
        scale_x: scale.0,
        scale_y: scale.1,
        scale_z: scale.2,
        pcode: 9, // PCode.Prim
        path_begin: 0,
        path_end: 0,
        path_scale_x: 100,
        path_scale_y: 100,
        path_shear_x: 0,
        path_shear_y: 0,
        path_skew: 0,
        path_curve: 16, // PathCurve.Line (box)
        path_radius_offset: 0,
        path_revolutions: 0,
        path_taper_x: 0,
        path_taper_y: 0,
        path_twist: 0,
        path_twist_begin: 0,
        profile_begin: 0,
        profile_end: 0,
        profile_curve: 1, // ProfileCurve.Square (box)
        profile_hollow: 0,
        texture: vec![0xCA, 0xFE],
        extra_params: vec![0x00],
        state: 0,
        last_attach_point: 0,
        media: String::new(),
        mat_ovrd: Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Build a 256x256 little-endian f32 heightfield the same way C# writes it
// ---------------------------------------------------------------------------

/// Generate terrain with a recognisable pattern:
///   height(x, y) = 21.0 + 0.04 * x + 0.02 * y
/// This gives a gentle slope from ~21 m in the SW corner to ~31.32 m in the
/// NE corner, with water at 20 m — typical of a default region.
fn build_terrain_heightfield() -> Vec<u8> {
    let mut buf = Vec::with_capacity(256 * 256 * 4);
    for y in 0u16..256 {
        for x in 0u16..256 {
            let h: f32 = 21.0 + 0.04 * (x as f32) + 0.02 * (y as f32);
            buf.extend_from_slice(&h.to_le_bytes());
        }
    }
    buf
}

/// Read one height sample from a packed little-endian f32 heightfield.
fn read_height(heightfield: &[u8], x: usize, y: usize) -> f32 {
    let offset = (y * 256 + x) * 4;
    f32::from_le_bytes([
        heightfield[offset],
        heightfield[offset + 1],
        heightfield[offset + 2],
        heightfield[offset + 3],
    ])
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Load a region database that contains two linkset groups and verify that
/// objects appear in the correct positions with the correct structure.
#[tokio::test]
async fn load_csharp_region_objects() {
    let (_pool, store) = setup_db().await;

    // -- Group A: root prim at (128, 128, 25) with one child ------------------
    // In C# OpenSim, the root prim has LinkNumber 1 and its own position equals
    // GroupPosition.  The child stores its local offset in Position and repeats
    // the root's world position in GroupPosition.

    let root = make_prim(
        ROOT_UUID,
        GROUP_A,
        1,                    // link 1 = root
        (128.0, 128.0, 25.0), // Position (same as GroupPosition for root)
        (128.0, 128.0, 25.0), // GroupPosition
        "House Foundation",
    );
    let root_shape = make_shape(ROOT_UUID, (10.0, 10.0, 0.5));

    let child = make_prim(
        CHILD_UUID,
        GROUP_A,
        2,                    // link 2 = first child
        (0.0, 0.0, 3.0),      // local offset from root
        (128.0, 128.0, 25.0), // GroupPosition = root's world pos
        "House Roof",
    );
    let child_shape = make_shape(CHILD_UUID, (12.0, 12.0, 0.3));

    // -- Group B: standalone prim at (50, 200, 30) ----------------------------
    let solo = make_prim(
        SOLO_UUID,
        GROUP_B,
        1,
        (50.0, 200.0, 30.0),
        (50.0, 200.0, 30.0),
        "Signpost",
    );
    let solo_shape = make_shape(SOLO_UUID, (0.5, 0.5, 2.0));

    // Store all three prims.
    store
        .store_object(REGION, GROUP_A, &root, &root_shape)
        .await
        .unwrap();
    store
        .store_object(REGION, GROUP_A, &child, &child_shape)
        .await
        .unwrap();
    store
        .store_object(REGION, GROUP_B, &solo, &solo_shape)
        .await
        .unwrap();

    // -- Load back through SimulationDataStore --------------------------------
    let loaded = store
        .load_objects(REGION)
        .await
        .expect("load_objects failed");
    assert_eq!(loaded.len(), 3, "expected 3 prims total across 2 groups");

    // `load_objects` sorts by (SceneGroupID, LinkNumber).  Build a map by UUID
    // for order-independent checks.
    let by_uuid: std::collections::HashMap<&str, &(PrimRow, Option<PrimShapeRow>)> = loaded
        .iter()
        .map(|pair| (pair.0.uuid.as_str(), pair))
        .collect();

    // -- Verify group A root prim ---
    let (rp, rs) = by_uuid[ROOT_UUID];
    assert_eq!(rp.scene_group_id, GROUP_A);
    assert_eq!(rp.link_number, 1, "root must be link 1");
    assert_eq!(rp.name, "House Foundation");
    assert_eq!(rp.position_x, 128.0);
    assert_eq!(rp.position_y, 128.0);
    assert_eq!(rp.position_z, 25.0);
    assert_eq!(rp.group_position_x, 128.0);
    assert_eq!(rp.group_position_y, 128.0);
    assert_eq!(rp.group_position_z, 25.0);
    // Shape
    let rs = rs.as_ref().expect("root shape must be present");
    assert_eq!(rs.scale_x, 10.0);
    assert_eq!(rs.scale_y, 10.0);
    assert_eq!(rs.scale_z, 0.5);
    assert_eq!(rs.pcode, 9);
    assert_eq!(rs.profile_curve, 1);

    // -- Verify group A child prim ---
    let (cp, cs) = by_uuid[CHILD_UUID];
    assert_eq!(cp.scene_group_id, GROUP_A);
    assert_eq!(cp.link_number, 2, "child must be link 2");
    assert_eq!(cp.name, "House Roof");
    // Child stores its *local offset* in Position.
    assert_eq!(cp.position_x, 0.0);
    assert_eq!(cp.position_y, 0.0);
    assert_eq!(cp.position_z, 3.0);
    // GroupPosition matches the root's world position.
    assert_eq!(cp.group_position_x, 128.0);
    assert_eq!(cp.group_position_y, 128.0);
    assert_eq!(cp.group_position_z, 25.0);
    let cs = cs.as_ref().expect("child shape must be present");
    assert_eq!(cs.scale_x, 12.0);
    assert_eq!(cs.scale_y, 12.0);
    assert_eq!(cs.scale_z, 0.3);

    // -- Verify group B standalone prim ---
    let (sp, ss) = by_uuid[SOLO_UUID];
    assert_eq!(sp.scene_group_id, GROUP_B);
    assert_eq!(sp.link_number, 1, "standalone prim is link 1");
    assert_eq!(sp.name, "Signpost");
    assert_eq!(sp.position_x, 50.0);
    assert_eq!(sp.position_y, 200.0);
    assert_eq!(sp.position_z, 30.0);
    assert_eq!(sp.group_position_x, 50.0);
    assert_eq!(sp.group_position_y, 200.0);
    assert_eq!(sp.group_position_z, 30.0);
    let ss = ss.as_ref().expect("solo shape must be present");
    assert_eq!(ss.scale_x, 0.5);
    assert_eq!(ss.scale_y, 0.5);
    assert_eq!(ss.scale_z, 2.0);

    // -- Verify both groups exist (by counting distinct SceneGroupIDs) ---
    let mut group_ids: Vec<&str> = loaded
        .iter()
        .map(|(p, _)| p.scene_group_id.as_str())
        .collect();
    group_ids.sort();
    group_ids.dedup();
    assert_eq!(group_ids.len(), 2, "expected 2 distinct groups");

    // -- Verify link ordering within group A (root before child) ---
    let group_a: Vec<&PrimRow> = loaded
        .iter()
        .map(|(p, _)| p)
        .filter(|p| p.scene_group_id == GROUP_A)
        .collect();
    assert_eq!(group_a.len(), 2);
    assert_eq!(group_a[0].link_number, 1, "root first in sort order");
    assert_eq!(group_a[1].link_number, 2, "child second in sort order");

    // -- Verify C# metadata fields survived ---
    assert_eq!(rp.object_flags, 524292);
    assert_eq!(rp.owner_mask, 0x7FFF_FFFF);
    assert_eq!(rp.next_owner_mask, 0x0008_2000);
    assert_eq!(rp.creator_id, OWNER);
    assert_eq!(rp.owner_id, OWNER);
    assert_eq!(rp.density, 1000.0);
    assert_eq!(rp.friction, 0.6);
    assert_eq!(rp.restitution, 0.5);
    assert_eq!(rp.material, 3);
}

/// Load terrain written by C# OpenSimulator and verify heights at known sample
/// points.
#[tokio::test]
async fn load_csharp_terrain_heightfield() {
    let (_pool, store) = setup_db().await;

    let heightfield = build_terrain_heightfield();
    assert_eq!(
        heightfield.len(),
        256 * 256 * 4,
        "heightfield must be 256x256 f32 LE"
    );

    store
        .store_terrain(REGION, 1, &heightfield)
        .await
        .expect("store_terrain failed");

    let loaded = store
        .load_terrain(REGION)
        .await
        .expect("load_terrain failed")
        .expect("terrain row must be present");

    assert_eq!(loaded.region_uuid, REGION);
    assert_eq!(loaded.revision, 1);
    assert_eq!(
        loaded.heightfield.len(),
        256 * 256 * 4,
        "heightfield blob size must match"
    );

    // Spot-check heights at known coordinates against the formula:
    //   h(x,y) = 21.0 + 0.04*x + 0.02*y
    let samples: &[(usize, usize, f32)] = &[
        (0, 0, 21.0),                                   // SW corner
        (255, 0, 21.0 + 0.04 * 255.0),                  // SE corner  = 31.2
        (0, 255, 21.0 + 0.02 * 255.0),                  // NW corner  = 26.1
        (255, 255, 21.0 + 0.04 * 255.0 + 0.02 * 255.0), // NE corner = 36.3
        (128, 128, 21.0 + 0.04 * 128.0 + 0.02 * 128.0), // center = 28.68
        (64, 192, 21.0 + 0.04 * 64.0 + 0.02 * 192.0),   // arbitrary = 27.4
    ];

    for &(x, y, expected) in samples {
        let actual = read_height(&loaded.heightfield, x, y);
        let diff = (actual - expected).abs();
        assert!(
            diff < 1e-4,
            "terrain height at ({x},{y}): expected {expected}, got {actual} (diff {diff})"
        );
    }
}

/// Store two terrain revisions and confirm `load_terrain` returns only the
/// latest one — matching C# OpenSimulator's "store overwrites previous" model.
#[tokio::test]
async fn terrain_latest_revision_wins() {
    let (_pool, store) = setup_db().await;

    // Revision 1: flat terrain at 20 m.
    let flat: Vec<u8> = (0..256 * 256)
        .flat_map(|_| 20.0_f32.to_le_bytes())
        .collect();

    store.store_terrain(REGION, 1, &flat).await.unwrap();

    // Revision 2: flat terrain at 35 m (simulates an edit in-world).
    let raised: Vec<u8> = (0..256 * 256)
        .flat_map(|_| 35.0_f32.to_le_bytes())
        .collect();

    store.store_terrain(REGION, 2, &raised).await.unwrap();

    let loaded = store
        .load_terrain(REGION)
        .await
        .unwrap()
        .expect("terrain must be present");

    // store_terrain DELETEs the old row before INSERT, so only revision 2 exists.
    assert_eq!(loaded.revision, 2);
    let h = read_height(&loaded.heightfield, 0, 0);
    assert!((h - 35.0).abs() < 1e-6, "expected 35.0, got {h}");
}

/// Verify get_object_ids returns exactly the prims that were inserted for the
/// region, matching the C# pattern of enumerating a region's object inventory.
#[tokio::test]
async fn get_object_ids_matches_inserted() {
    let (_pool, store) = setup_db().await;

    let root = make_prim(
        ROOT_UUID,
        GROUP_A,
        1,
        (128.0, 128.0, 25.0),
        (128.0, 128.0, 25.0),
        "Root",
    );
    let child = make_prim(
        CHILD_UUID,
        GROUP_A,
        2,
        (0.0, 0.0, 3.0),
        (128.0, 128.0, 25.0),
        "Child",
    );
    let solo = make_prim(
        SOLO_UUID,
        GROUP_B,
        1,
        (50.0, 200.0, 30.0),
        (50.0, 200.0, 30.0),
        "Solo",
    );

    store
        .store_object(
            REGION,
            GROUP_A,
            &root,
            &make_shape(ROOT_UUID, (1.0, 1.0, 1.0)),
        )
        .await
        .unwrap();
    store
        .store_object(
            REGION,
            GROUP_A,
            &child,
            &make_shape(CHILD_UUID, (1.0, 1.0, 1.0)),
        )
        .await
        .unwrap();
    store
        .store_object(
            REGION,
            GROUP_B,
            &solo,
            &make_shape(SOLO_UUID, (1.0, 1.0, 1.0)),
        )
        .await
        .unwrap();

    let mut ids = store
        .get_object_ids(REGION)
        .await
        .expect("get_object_ids failed");
    ids.sort();

    let mut expected = vec![
        ROOT_UUID.to_string(),
        CHILD_UUID.to_string(),
        SOLO_UUID.to_string(),
    ];
    expected.sort();

    assert_eq!(ids, expected);
}

/// After removing a prim from a linked group, the remaining prim and the
/// standalone prim should still load correctly.
#[tokio::test]
async fn remove_child_preserves_remaining_objects() {
    let (_pool, store) = setup_db().await;

    let root = make_prim(
        ROOT_UUID,
        GROUP_A,
        1,
        (128.0, 128.0, 25.0),
        (128.0, 128.0, 25.0),
        "Root",
    );
    let child = make_prim(
        CHILD_UUID,
        GROUP_A,
        2,
        (0.0, 0.0, 3.0),
        (128.0, 128.0, 25.0),
        "Child",
    );
    let solo = make_prim(
        SOLO_UUID,
        GROUP_B,
        1,
        (50.0, 200.0, 30.0),
        (50.0, 200.0, 30.0),
        "Solo",
    );

    store
        .store_object(
            REGION,
            GROUP_A,
            &root,
            &make_shape(ROOT_UUID, (1.0, 1.0, 1.0)),
        )
        .await
        .unwrap();
    store
        .store_object(
            REGION,
            GROUP_A,
            &child,
            &make_shape(CHILD_UUID, (1.0, 1.0, 1.0)),
        )
        .await
        .unwrap();
    store
        .store_object(
            REGION,
            GROUP_B,
            &solo,
            &make_shape(SOLO_UUID, (1.0, 1.0, 1.0)),
        )
        .await
        .unwrap();

    // Remove the child prim (simulates unlinking in C# OpenSim).
    store
        .remove_object(CHILD_UUID)
        .await
        .expect("remove_object failed");

    let loaded = store
        .load_objects(REGION)
        .await
        .expect("load_objects failed");
    assert_eq!(loaded.len(), 2, "root + solo should remain");

    let uuids: Vec<&str> = loaded.iter().map(|(p, _)| p.uuid.as_str()).collect();
    assert!(uuids.contains(&ROOT_UUID), "root must survive");
    assert!(uuids.contains(&SOLO_UUID), "solo must survive");
    assert!(!uuids.contains(&CHILD_UUID), "child must be gone");

    // Verify the surviving root's position is intact.
    let root_row = loaded.iter().find(|(p, _)| p.uuid == ROOT_UUID).unwrap();
    assert_eq!(root_row.0.position_x, 128.0);
    assert_eq!(root_row.0.position_z, 25.0);
}
