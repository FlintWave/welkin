// SPDX-License-Identifier: LGPL-3.0-or-later

//! Round-trip integration tests for `SimulationDataStore`.
//!
//! Each test creates an in-memory SQLite database with the C# OpenSimulator
//! schema and verifies that the Rust code can store and load data correctly.

use sqlx::AnyPool;
use sqlx::any::AnyPoolOptions;
use welkin_data::simulation::*;

// ---------------------------------------------------------------------------
// Schema & helpers
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

const REGION: &str = "550e8400-e29b-41d4-a716-446655440000";
const ZERO: &str = "00000000-0000-0000-0000-000000000000";

/// Create an in-memory SQLite pool with the C# OpenSimulator schema applied.
async fn setup_db() -> (AnyPool, SimulationDataStore) {
    sqlx::any::install_default_drivers();
    let pool = AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("failed to connect to in-memory SQLite");

    // Execute each CREATE TABLE statement separately (sqlx doesn't support
    // multiple statements in one `execute` with the Any driver).
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

/// Construct a realistic `PrimRow` with the given UUID and link number.
fn make_prim(uuid: &str, link_number: i32) -> PrimRow {
    PrimRow {
        uuid: uuid.into(),
        region_uuid: REGION.into(),
        scene_group_id: "a1b2c3d4-e5f6-7890-abcd-ef1234567890".into(),
        creation_date: 1_700_000_000,
        name: "Test Cube".into(),
        text: "hover text".into(),
        description: "A test prim".into(),
        sit_name: "Sit Here".into(),
        touch_name: "Touch Me".into(),
        object_flags: 524292,
        creator_id: "d47f3c77-9e2a-4b58-b6d1-8e3f2a1c5b09".into(),
        owner_id: "d47f3c77-9e2a-4b58-b6d1-8e3f2a1c5b09".into(),
        group_id: ZERO.into(),
        last_owner_id: "d47f3c77-9e2a-4b58-b6d1-8e3f2a1c5b09".into(),
        rezzer_id: ZERO.into(),
        owner_mask: 0x7FFF_FFFF,
        next_owner_mask: 0x0008_2000,
        group_mask: 0,
        everyone_mask: 0,
        base_mask: 0x7FFF_FFFF,
        position_x: 128.5,
        position_y: 64.25,
        position_z: 33.0,
        group_position_x: 128.5,
        group_position_y: 64.25,
        group_position_z: 33.0,
        velocity_x: 0.0,
        velocity_y: 0.0,
        velocity_z: 0.0,
        angular_velocity_x: 0.1,
        angular_velocity_y: 0.0,
        angular_velocity_z: 0.0,
        acceleration_x: 0.0,
        acceleration_y: 0.0,
        acceleration_z: -9.8,
        rotation_x: 0.0,
        rotation_y: 0.0,
        rotation_z: 0.707_107,
        rotation_w: 0.707_107,
        sit_target_offset_x: 0.0,
        sit_target_offset_y: 0.0,
        sit_target_offset_z: 0.5,
        sit_target_orient_w: 1.0,
        sit_target_orient_x: 0.0,
        sit_target_orient_y: 0.0,
        sit_target_orient_z: 0.0,
        pay_price: -2,
        pay_button1: 5,
        pay_button2: 10,
        pay_button3: 20,
        pay_button4: 50,
        looped_sound: ZERO.into(),
        looped_sound_gain: 0.0,
        texture_animation: vec![0xDE, 0xAD, 0xBE, 0xEF],
        camera_eye_offset_x: 0.0,
        camera_eye_offset_y: 0.0,
        camera_eye_offset_z: 3.0,
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
        color_g: 128,
        color_b: 0,
        color_a: 255,
        particle_system: vec![0x01, 0x02, 0x03],
        click_action: 0,
        material: 3,
        collision_sound: ZERO.into(),
        collision_sound_volume: 0.0,
        pass_touches: 0,
        pass_collisions: 0,
        link_number,
        media_url: String::new(),
        physics_shape_type: 2,
        density: 1000.0,
        gravity_modifier: 1.0,
        friction: 0.6,
        restitution: 0.5,
        dyn_attrs: "{\"test\":1}".into(),
    }
}

/// Construct a realistic `PrimShapeRow` with the given UUID.
fn make_shape(uuid: &str) -> PrimShapeRow {
    PrimShapeRow {
        uuid: uuid.into(),
        shape: 0,
        scale_x: 0.5,
        scale_y: 1.0,
        scale_z: 2.0,
        pcode: 9,
        path_begin: 0,
        path_end: 0,
        path_scale_x: 100,
        path_scale_y: 100,
        path_shear_x: 0,
        path_shear_y: 0,
        path_skew: 0,
        path_curve: 16,
        path_radius_offset: 0,
        path_revolutions: 0,
        path_taper_x: 0,
        path_taper_y: 0,
        path_twist: 0,
        path_twist_begin: 0,
        profile_begin: 0,
        profile_end: 0,
        profile_curve: 1,
        profile_hollow: 0,
        texture: vec![0xCA, 0xFE, 0xBA, 0xBE],
        extra_params: vec![0x00],
        state: 0,
        last_attach_point: 0,
        media: String::new(),
        mat_ovrd: Vec::new(),
    }
}

/// Construct a realistic `RegionSettingsRow`.
fn make_settings() -> RegionSettingsRow {
    RegionSettingsRow {
        region_uuid: REGION.into(),
        block_terraform: 0,
        block_fly: 0,
        allow_damage: 1,
        restrict_pushing: 0,
        allow_land_resell: 1,
        allow_land_join_divide: 1,
        block_show_in_search: 0,
        agent_limit: 40,
        object_bonus: 1.5,
        maturity: 2,
        disable_scripts: 0,
        disable_collisions: 0,
        disable_physics: 0,
        terrain_texture_1: "b8d3965a-d018-4b3c-b1c4-2e3f7c5a9d01".into(),
        terrain_texture_2: "abb783e6-3e93-4c1a-b65e-7b2f4c8d3e02".into(),
        terrain_texture_3: "179cdabd-398a-4e06-8986-d7c3b2a1f003".into(),
        terrain_texture_4: "beb169c7-11ea-4722-a2ce-e4543b1d7004".into(),
        elevation_1_nw: 10.0,
        elevation_1_ne: 10.0,
        elevation_1_se: 10.0,
        elevation_1_sw: 10.0,
        elevation_2_nw: 60.0,
        elevation_2_ne: 60.0,
        elevation_2_se: 60.0,
        elevation_2_sw: 60.0,
        water_height: 20.0,
        terrain_raise_limit: 100.0,
        terrain_lower_limit: -100.0,
        use_estate_sun: 1,
        fixed_sun: 0,
        sun_position: 0.0,
        covenant: ZERO.into(),
        covenant_datetime: 1_700_000_000,
        sandbox: 0,
        map_tile_id: ZERO.into(),
        telehub_object: ZERO.into(),
        parcel_tile_id: ZERO.into(),
        cache_id: "c1d2e3f4-a5b6-7890-cdef-123456789abc".into(),
        block_search: 0,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_object_roundtrip() {
    let (_pool, store) = setup_db().await;

    let prim = make_prim("550e8400-e29b-41d4-a716-446655440001", 1);
    let shape = make_shape("550e8400-e29b-41d4-a716-446655440001");

    store
        .store_object(REGION, &prim.scene_group_id, &prim, &shape)
        .await
        .expect("store_object failed");

    let loaded = store
        .load_objects(REGION)
        .await
        .expect("load_objects failed");

    assert_eq!(loaded.len(), 1, "expected exactly one prim");
    let (lp, ls) = &loaded[0];

    // Verify all PrimRow fields round-trip correctly.
    assert_eq!(lp.uuid, prim.uuid);
    assert_eq!(lp.region_uuid, prim.region_uuid);
    assert_eq!(lp.scene_group_id, prim.scene_group_id);
    assert_eq!(lp.creation_date, prim.creation_date);
    assert_eq!(lp.name, prim.name);
    assert_eq!(lp.text, prim.text);
    assert_eq!(lp.description, prim.description);
    assert_eq!(lp.sit_name, prim.sit_name);
    assert_eq!(lp.touch_name, prim.touch_name);
    assert_eq!(lp.object_flags, prim.object_flags);
    assert_eq!(lp.creator_id, prim.creator_id);
    assert_eq!(lp.owner_id, prim.owner_id);
    assert_eq!(lp.group_id, prim.group_id);
    assert_eq!(lp.last_owner_id, prim.last_owner_id);
    assert_eq!(lp.rezzer_id, prim.rezzer_id);
    assert_eq!(lp.owner_mask, prim.owner_mask);
    assert_eq!(lp.next_owner_mask, prim.next_owner_mask);
    assert_eq!(lp.group_mask, prim.group_mask);
    assert_eq!(lp.everyone_mask, prim.everyone_mask);
    assert_eq!(lp.base_mask, prim.base_mask);
    assert_eq!(lp.position_x, prim.position_x);
    assert_eq!(lp.position_y, prim.position_y);
    assert_eq!(lp.position_z, prim.position_z);
    assert_eq!(lp.group_position_x, prim.group_position_x);
    assert_eq!(lp.group_position_y, prim.group_position_y);
    assert_eq!(lp.group_position_z, prim.group_position_z);
    assert_eq!(lp.velocity_x, prim.velocity_x);
    assert_eq!(lp.velocity_y, prim.velocity_y);
    assert_eq!(lp.velocity_z, prim.velocity_z);
    assert_eq!(lp.angular_velocity_x, prim.angular_velocity_x);
    assert_eq!(lp.angular_velocity_y, prim.angular_velocity_y);
    assert_eq!(lp.angular_velocity_z, prim.angular_velocity_z);
    assert_eq!(lp.acceleration_x, prim.acceleration_x);
    assert_eq!(lp.acceleration_y, prim.acceleration_y);
    assert_eq!(lp.acceleration_z, prim.acceleration_z);
    assert_eq!(lp.rotation_x, prim.rotation_x);
    assert_eq!(lp.rotation_y, prim.rotation_y);
    assert_eq!(lp.rotation_z, prim.rotation_z);
    assert_eq!(lp.rotation_w, prim.rotation_w);
    assert_eq!(lp.sit_target_offset_x, prim.sit_target_offset_x);
    assert_eq!(lp.sit_target_offset_y, prim.sit_target_offset_y);
    assert_eq!(lp.sit_target_offset_z, prim.sit_target_offset_z);
    assert_eq!(lp.sit_target_orient_w, prim.sit_target_orient_w);
    assert_eq!(lp.sit_target_orient_x, prim.sit_target_orient_x);
    assert_eq!(lp.sit_target_orient_y, prim.sit_target_orient_y);
    assert_eq!(lp.sit_target_orient_z, prim.sit_target_orient_z);
    assert_eq!(lp.pay_price, prim.pay_price);
    assert_eq!(lp.pay_button1, prim.pay_button1);
    assert_eq!(lp.pay_button2, prim.pay_button2);
    assert_eq!(lp.pay_button3, prim.pay_button3);
    assert_eq!(lp.pay_button4, prim.pay_button4);
    assert_eq!(lp.looped_sound, prim.looped_sound);
    assert_eq!(lp.looped_sound_gain, prim.looped_sound_gain);
    assert_eq!(lp.texture_animation, prim.texture_animation);
    assert_eq!(lp.camera_eye_offset_x, prim.camera_eye_offset_x);
    assert_eq!(lp.camera_eye_offset_y, prim.camera_eye_offset_y);
    assert_eq!(lp.camera_eye_offset_z, prim.camera_eye_offset_z);
    assert_eq!(lp.camera_at_offset_x, prim.camera_at_offset_x);
    assert_eq!(lp.camera_at_offset_y, prim.camera_at_offset_y);
    assert_eq!(lp.camera_at_offset_z, prim.camera_at_offset_z);
    assert_eq!(lp.force_mouselook, prim.force_mouselook);
    assert_eq!(lp.script_access_pin, prim.script_access_pin);
    assert_eq!(lp.allowed_drop, prim.allowed_drop);
    assert_eq!(lp.die_at_edge, prim.die_at_edge);
    assert_eq!(lp.sale_price, prim.sale_price);
    assert_eq!(lp.sale_type, prim.sale_type);
    assert_eq!(lp.color_r, prim.color_r);
    assert_eq!(lp.color_g, prim.color_g);
    assert_eq!(lp.color_b, prim.color_b);
    assert_eq!(lp.color_a, prim.color_a);
    assert_eq!(lp.particle_system, prim.particle_system);
    assert_eq!(lp.click_action, prim.click_action);
    assert_eq!(lp.material, prim.material);
    assert_eq!(lp.collision_sound, prim.collision_sound);
    assert_eq!(lp.collision_sound_volume, prim.collision_sound_volume);
    assert_eq!(lp.pass_touches, prim.pass_touches);
    assert_eq!(lp.pass_collisions, prim.pass_collisions);
    assert_eq!(lp.link_number, prim.link_number);
    assert_eq!(lp.media_url, prim.media_url);
    assert_eq!(lp.physics_shape_type, prim.physics_shape_type);
    assert_eq!(lp.density, prim.density);
    assert_eq!(lp.gravity_modifier, prim.gravity_modifier);
    assert_eq!(lp.friction, prim.friction);
    assert_eq!(lp.restitution, prim.restitution);
    assert_eq!(lp.dyn_attrs, prim.dyn_attrs);

    // Verify PrimShapeRow round-trips.
    let ls = ls.as_ref().expect("shape should be present");
    assert_eq!(ls.uuid, shape.uuid);
    assert_eq!(ls.shape, shape.shape);
    assert_eq!(ls.scale_x, shape.scale_x);
    assert_eq!(ls.scale_y, shape.scale_y);
    assert_eq!(ls.scale_z, shape.scale_z);
    assert_eq!(ls.pcode, shape.pcode);
    assert_eq!(ls.path_begin, shape.path_begin);
    assert_eq!(ls.path_end, shape.path_end);
    assert_eq!(ls.path_scale_x, shape.path_scale_x);
    assert_eq!(ls.path_scale_y, shape.path_scale_y);
    assert_eq!(ls.path_shear_x, shape.path_shear_x);
    assert_eq!(ls.path_shear_y, shape.path_shear_y);
    assert_eq!(ls.path_skew, shape.path_skew);
    assert_eq!(ls.path_curve, shape.path_curve);
    assert_eq!(ls.path_radius_offset, shape.path_radius_offset);
    assert_eq!(ls.path_revolutions, shape.path_revolutions);
    assert_eq!(ls.path_taper_x, shape.path_taper_x);
    assert_eq!(ls.path_taper_y, shape.path_taper_y);
    assert_eq!(ls.path_twist, shape.path_twist);
    assert_eq!(ls.path_twist_begin, shape.path_twist_begin);
    assert_eq!(ls.profile_begin, shape.profile_begin);
    assert_eq!(ls.profile_end, shape.profile_end);
    assert_eq!(ls.profile_curve, shape.profile_curve);
    assert_eq!(ls.profile_hollow, shape.profile_hollow);
    assert_eq!(ls.texture, shape.texture);
    assert_eq!(ls.extra_params, shape.extra_params);
    assert_eq!(ls.state, shape.state);
    assert_eq!(ls.last_attach_point, shape.last_attach_point);
    assert_eq!(ls.media, shape.media);
    assert_eq!(ls.mat_ovrd, shape.mat_ovrd);
}

#[tokio::test]
async fn test_terrain_roundtrip() {
    let (_pool, store) = setup_db().await;

    // Build a 256x256 f32 little-endian heightfield (matching C# format).
    let mut heightfield = Vec::with_capacity(256 * 256 * 4);
    for y in 0u16..256 {
        for x in 0u16..256 {
            // Gentle slope: height = 20.0 + x*0.1 + y*0.05
            let h: f32 = 20.0 + (x as f32) * 0.1 + (y as f32) * 0.05;
            heightfield.extend_from_slice(&h.to_le_bytes());
        }
    }
    assert_eq!(heightfield.len(), 256 * 256 * 4);

    store
        .store_terrain(REGION, 42, &heightfield)
        .await
        .expect("store_terrain failed");

    let loaded = store
        .load_terrain(REGION)
        .await
        .expect("load_terrain failed")
        .expect("terrain row should be present");

    assert_eq!(loaded.region_uuid, REGION);
    assert_eq!(loaded.revision, 42);
    assert_eq!(
        loaded.heightfield, heightfield,
        "heightfield bytes must match exactly"
    );
}

#[tokio::test]
async fn test_region_settings_roundtrip() {
    let (_pool, store) = setup_db().await;

    let settings = make_settings();

    store
        .store_region_settings(&settings)
        .await
        .expect("store_region_settings failed");

    let loaded = store
        .load_region_settings(REGION)
        .await
        .expect("load_region_settings failed")
        .expect("settings should be present");

    assert_eq!(loaded.region_uuid, settings.region_uuid);
    assert_eq!(loaded.block_terraform, settings.block_terraform);
    assert_eq!(loaded.block_fly, settings.block_fly);
    assert_eq!(loaded.allow_damage, settings.allow_damage);
    assert_eq!(loaded.restrict_pushing, settings.restrict_pushing);
    assert_eq!(loaded.allow_land_resell, settings.allow_land_resell);
    assert_eq!(
        loaded.allow_land_join_divide,
        settings.allow_land_join_divide
    );
    assert_eq!(loaded.block_show_in_search, settings.block_show_in_search);
    assert_eq!(loaded.agent_limit, settings.agent_limit);
    assert_eq!(loaded.object_bonus, settings.object_bonus);
    assert_eq!(loaded.maturity, settings.maturity);
    assert_eq!(loaded.disable_scripts, settings.disable_scripts);
    assert_eq!(loaded.disable_collisions, settings.disable_collisions);
    assert_eq!(loaded.disable_physics, settings.disable_physics);
    assert_eq!(loaded.terrain_texture_1, settings.terrain_texture_1);
    assert_eq!(loaded.terrain_texture_2, settings.terrain_texture_2);
    assert_eq!(loaded.terrain_texture_3, settings.terrain_texture_3);
    assert_eq!(loaded.terrain_texture_4, settings.terrain_texture_4);
    assert_eq!(loaded.elevation_1_nw, settings.elevation_1_nw);
    assert_eq!(loaded.elevation_1_ne, settings.elevation_1_ne);
    assert_eq!(loaded.elevation_1_se, settings.elevation_1_se);
    assert_eq!(loaded.elevation_1_sw, settings.elevation_1_sw);
    assert_eq!(loaded.elevation_2_nw, settings.elevation_2_nw);
    assert_eq!(loaded.elevation_2_ne, settings.elevation_2_ne);
    assert_eq!(loaded.elevation_2_se, settings.elevation_2_se);
    assert_eq!(loaded.elevation_2_sw, settings.elevation_2_sw);
    assert_eq!(loaded.water_height, settings.water_height);
    assert_eq!(loaded.terrain_raise_limit, settings.terrain_raise_limit);
    assert_eq!(loaded.terrain_lower_limit, settings.terrain_lower_limit);
    assert_eq!(loaded.use_estate_sun, settings.use_estate_sun);
    assert_eq!(loaded.fixed_sun, settings.fixed_sun);
    assert_eq!(loaded.sun_position, settings.sun_position);
    assert_eq!(loaded.covenant, settings.covenant);
    assert_eq!(loaded.covenant_datetime, settings.covenant_datetime);
    assert_eq!(loaded.sandbox, settings.sandbox);
    assert_eq!(loaded.map_tile_id, settings.map_tile_id);
    assert_eq!(loaded.telehub_object, settings.telehub_object);
    assert_eq!(loaded.parcel_tile_id, settings.parcel_tile_id);
    assert_eq!(loaded.cache_id, settings.cache_id);
    assert_eq!(loaded.block_search, settings.block_search);
}

#[tokio::test]
async fn test_remove_object() {
    let (_pool, store) = setup_db().await;

    let uuid = "550e8400-e29b-41d4-a716-446655440002";
    let prim = make_prim(uuid, 1);
    let shape = make_shape(uuid);

    store
        .store_object(REGION, &prim.scene_group_id, &prim, &shape)
        .await
        .expect("store_object failed");

    // Verify it exists.
    let before = store.load_objects(REGION).await.expect("load failed");
    assert_eq!(before.len(), 1);

    // Remove it.
    store
        .remove_object(uuid)
        .await
        .expect("remove_object failed");

    // Verify it is gone.
    let after = store.load_objects(REGION).await.expect("load failed");
    assert!(after.is_empty(), "objects should be empty after removal");

    // Also check get_object_ids returns empty.
    let ids = store
        .get_object_ids(REGION)
        .await
        .expect("get_object_ids failed");
    assert!(ids.is_empty());
}

#[tokio::test]
async fn test_multiple_objects_in_group() {
    let (_pool, store) = setup_db().await;

    let group_id = "a1b2c3d4-e5f6-7890-abcd-ef1234567890";

    // Store 3 prims with the same SceneGroupID but different link numbers.
    // Insert out of order to test that load_objects sorts by LinkNumber.
    let uuids = [
        ("550e8400-e29b-41d4-a716-446655440010", 3),
        ("550e8400-e29b-41d4-a716-446655440011", 1),
        ("550e8400-e29b-41d4-a716-446655440012", 2),
    ];

    for (uuid, link_num) in &uuids {
        let mut prim = make_prim(uuid, *link_num);
        prim.scene_group_id = group_id.into();
        let shape = make_shape(uuid);
        store
            .store_object(REGION, group_id, &prim, &shape)
            .await
            .unwrap_or_else(|e| panic!("store_object({uuid}) failed: {e}"));
    }

    let loaded = store
        .load_objects(REGION)
        .await
        .expect("load_objects failed");

    assert_eq!(loaded.len(), 3, "expected 3 prims in group");

    // Verify they come back sorted by LinkNumber (the ORDER BY in load_objects).
    assert_eq!(loaded[0].0.link_number, 1);
    assert_eq!(loaded[1].0.link_number, 2);
    assert_eq!(loaded[2].0.link_number, 3);

    // Verify each has its shape.
    for (p, s) in &loaded {
        let s = s.as_ref().expect("shape should be present");
        assert_eq!(p.uuid, s.uuid, "prim/shape UUID mismatch");
    }

    // Verify all share the same SceneGroupID.
    for (p, _) in &loaded {
        assert_eq!(p.scene_group_id, group_id);
    }

    // Verify get_object_ids returns all 3.
    let ids = store
        .get_object_ids(REGION)
        .await
        .expect("get_object_ids failed");
    assert_eq!(ids.len(), 3);
}
