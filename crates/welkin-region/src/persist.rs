// SPDX-License-Identifier: LGPL-3.0-or-later

use welkin_data::simulation::{PrimRow, PrimShapeRow, SimulationDataStore};
use welkin_scene::part::SceneObjectPart;
use welkin_scene::scene::Scene;

pub fn part_to_prim_row(part: &SceneObjectPart, region_uuid: &str, group_id: &str) -> PrimRow {
    PrimRow {
        uuid: part.uuid.to_string(),
        region_uuid: region_uuid.to_string(),
        scene_group_id: group_id.to_string(),
        creation_date: part.creation_date,
        name: part.name.clone(),
        text: part.text.clone(),
        description: part.description.clone(),
        sit_name: part.sit_name.clone(),
        touch_name: part.touch_name.clone(),
        object_flags: part.flags.bits() as i32,
        creator_id: part.creator_id.to_string(),
        owner_id: part.owner_id.to_string(),
        group_id: part.group_id.to_string(),
        last_owner_id: part.last_owner_id.to_string(),
        rezzer_id: part.rezzer_id.to_string(),
        owner_mask: part.owner_mask.bits() as i32,
        next_owner_mask: part.next_owner_mask.bits() as i32,
        group_mask: part.group_mask.bits() as i32,
        everyone_mask: part.everyone_mask.bits() as i32,
        base_mask: part.base_mask.bits() as i32,
        position_x: part.offset_position.x as f64,
        position_y: part.offset_position.y as f64,
        position_z: part.offset_position.z as f64,
        group_position_x: part.group_position.x as f64,
        group_position_y: part.group_position.y as f64,
        group_position_z: part.group_position.z as f64,
        velocity_x: part.velocity.x as f64,
        velocity_y: part.velocity.y as f64,
        velocity_z: part.velocity.z as f64,
        angular_velocity_x: part.angular_velocity.x as f64,
        angular_velocity_y: part.angular_velocity.y as f64,
        angular_velocity_z: part.angular_velocity.z as f64,
        acceleration_x: part.acceleration.x as f64,
        acceleration_y: part.acceleration.y as f64,
        acceleration_z: part.acceleration.z as f64,
        rotation_x: part.rotation_offset.x as f64,
        rotation_y: part.rotation_offset.y as f64,
        rotation_z: part.rotation_offset.z as f64,
        rotation_w: part.rotation_offset.w as f64,
        sit_target_offset_x: part.sit_target_position.x as f64,
        sit_target_offset_y: part.sit_target_position.y as f64,
        sit_target_offset_z: part.sit_target_position.z as f64,
        sit_target_orient_w: part.sit_target_orientation.w as f64,
        sit_target_orient_x: part.sit_target_orientation.x as f64,
        sit_target_orient_y: part.sit_target_orientation.y as f64,
        sit_target_orient_z: part.sit_target_orientation.z as f64,
        pay_price: part.pay_price[0],
        pay_button1: part.pay_price[1],
        pay_button2: part.pay_price[2],
        pay_button3: part.pay_price[3],
        pay_button4: part.pay_price[4],
        looped_sound: part.sound_id.to_string(),
        looped_sound_gain: part.sound_gain as f64,
        texture_animation: part.texture_animation.clone(),
        camera_eye_offset_x: part.camera_eye_offset.x as f64,
        camera_eye_offset_y: part.camera_eye_offset.y as f64,
        camera_eye_offset_z: part.camera_eye_offset.z as f64,
        camera_at_offset_x: part.camera_at_offset.x as f64,
        camera_at_offset_y: part.camera_at_offset.y as f64,
        camera_at_offset_z: part.camera_at_offset.z as f64,
        force_mouselook: 0,
        script_access_pin: part.script_access_pin,
        allowed_drop: i32::from(part.allowed_drop),
        die_at_edge: i32::from(part.die_at_edge),
        sale_price: part.sale_price,
        sale_type: part.object_sale_type as i32,
        color_r: part.text_color.r() as i32,
        color_g: part.text_color.g() as i32,
        color_b: part.text_color.b() as i32,
        color_a: part.text_color.a() as i32,
        particle_system: part.particle_system.clone(),
        click_action: part.click_action as i32,
        material: part.material as i32,
        collision_sound: part.collision_sound.to_string(),
        collision_sound_volume: part.collision_sound_volume as f64,
        pass_touches: i32::from(part.pass_touches),
        pass_collisions: i32::from(part.pass_collisions),
        link_number: part.link_num,
        media_url: part.media_url.clone(),
        physics_shape_type: part.physics_shape_type as i32,
        density: part.density as f64,
        gravity_modifier: part.gravity_modifier as f64,
        friction: part.friction as f64,
        restitution: part.bounce as f64,
        dyn_attrs: String::new(),
    }
}

pub fn shape_to_row(part: &SceneObjectPart) -> PrimShapeRow {
    let s = &part.shape;
    PrimShapeRow {
        uuid: part.uuid.to_string(),
        shape: 0,
        scale_x: part.scale.x as f64,
        scale_y: part.scale.y as f64,
        scale_z: part.scale.z as f64,
        pcode: s.pcode as i32,
        path_begin: s.path_begin as i32,
        path_end: s.path_end as i32,
        path_scale_x: s.path_scale_x as i32,
        path_scale_y: s.path_scale_y as i32,
        path_shear_x: s.path_shear_x as i32,
        path_shear_y: s.path_shear_y as i32,
        path_skew: s.path_skew as i32,
        path_curve: s.path_curve as i32,
        path_radius_offset: s.path_radius_offset as i32,
        path_revolutions: s.path_revolutions as i32,
        path_taper_x: s.path_taper_x as i32,
        path_taper_y: s.path_taper_y as i32,
        path_twist: s.path_twist as i32,
        path_twist_begin: s.path_twist_begin as i32,
        profile_begin: s.profile_begin as i32,
        profile_end: s.profile_end as i32,
        profile_curve: s.profile_curve as i32,
        profile_hollow: s.profile_hollow as i32,
        texture: s.texture_entry.clone(),
        extra_params: s.extra_params.clone(),
        state: s.state as i32,
        last_attach_point: s.last_attach_point as i32,
        media: s.media.clone(),
        mat_ovrd: s.mat_override.clone(),
    }
}

pub async fn backup_dirty_objects(
    scene: &Scene,
    store: &SimulationDataStore,
    region_uuid: &str,
) -> usize {
    let dirty_ids = scene.collect_for_backup();
    let mut persisted = 0usize;
    for uuid in &dirty_ids {
        if let Some(group) = scene.get_object(uuid) {
            let gid = group.uuid().to_string();
            let mut group_ok = true;
            for part in group.parts() {
                let prim_row = part_to_prim_row(part, region_uuid, &gid);
                let shape_row = shape_to_row(part);
                if let Err(e) = store.store_object(region_uuid, &gid, &prim_row, &shape_row).await
                {
                    tracing::warn!(
                        uuid = %part.uuid,
                        error = %e,
                        "failed to persist prim"
                    );
                    group_ok = false;
                } else {
                    persisted += 1;
                }
            }
            if group_ok {
                scene.mark_backed_up(uuid);
            }
        }
    }
    persisted
}

/// Save the terrain if it was edited since the last save. Returns true if saved.
pub async fn backup_terrain(scene: &Scene, store: &SimulationDataStore, region_uuid: &str) -> bool {
    let Some(heights) = scene.take_terrain_for_backup() else {
        return false;
    };
    let blob = match welkin_scene::terrain_io::to_db_blob(256, 256, &heights) {
        Ok(blob) => blob,
        Err(e) => {
            tracing::error!(error = %e, "terrain not saved: serialisation failed");
            scene.mark_terrain_modified();
            return false;
        }
    };
    match store
        .store_terrain(region_uuid, welkin_scene::terrain_io::DB_REVISION_VARIABLE_2D_GZIP, &blob)
        .await
    {
        Ok(()) => true,
        Err(e) => {
            tracing::error!(error = %e, "terrain not saved: database write failed");
            scene.mark_terrain_modified();
            false
        }
    }
}

/// Terrain heights saved for this region, if any (and usable for a 256 m region).
pub async fn load_saved_terrain(store: &SimulationDataStore, region_uuid: &str) -> Option<Vec<f32>> {
    let row = match store.load_terrain(region_uuid).await {
        Ok(Some(row)) => row,
        Ok(None) => return None,
        Err(e) => {
            tracing::error!(error = %e, "could not read saved terrain; starting with flat terrain");
            return None;
        }
    };
    match welkin_scene::terrain_io::from_db_blob(row.revision, &row.heightfield) {
        Ok((256, 256, heights)) => Some(heights),
        Ok((w, h, _)) => {
            tracing::error!(w, h, "saved terrain is not 256x256; ignoring it");
            None
        }
        Err(e) => {
            tracing::error!(revision = row.revision, error = %e, "saved terrain is unreadable; ignoring it");
            None
        }
    }
}

#[cfg(test)]
mod terrain_tests {
    use super::*;
    use welkin_scene::scene::{RegionInfo, SceneConfig};
    use welkin_scene::terrain_paint::PaintAction;

    async fn store() -> (SimulationDataStore, std::path::PathBuf) {
        sqlx::any::install_default_drivers();
        // A file: each connection to `sqlite::memory:` would be a separate database.
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let path = std::env::temp_dir().join(format!(
            "welkin-terrain-test-{}-{}.db",
            std::process::id(),
            N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        ));
        let _ = std::fs::remove_file(&path);
        let url = format!("sqlite:{}?mode=rwc", path.display());
        let pool = welkin_data::DataPool::connect(&url).await.expect("pool");
        let runner = welkin_data::migrations::MigrationRunner::new(
            pool.inner().clone(),
            welkin_data::migrations::Backend::SQLite,
        );
        let sql = welkin_data::migrations::migration_content(
            "RegionStore",
            welkin_data::migrations::Backend::SQLite,
        )
        .expect("RegionStore migrations");
        runner.ensure_migrations_table().await.expect("migrations table");
        runner.run_migrations("RegionStore", sql).await.expect("region migrations");
        (SimulationDataStore::new(pool.inner().clone()), path)
    }

    #[tokio::test]
    async fn edited_terrain_survives_a_restart() {
        let (store, db_path) = store().await;
        let region = "11111111-1111-1111-1111-111111111111";
        assert!(load_saved_terrain(&store, region).await.is_none());

        let scene = Scene::new(RegionInfo::new("T", 1000, 1000), SceneConfig::default());
        assert!(!backup_terrain(&scene, &store, region).await, "nothing to save yet");
        scene.modify_terrain(PaintAction::Raise, 100.0, 100.0, 0.0, 4.0, 3.0);
        let edited = scene.terrain_height_at(100.0, 100.0);
        assert!(backup_terrain(&scene, &store, region).await);
        assert!(!backup_terrain(&scene, &store, region).await, "saved once");

        // "Restart": a fresh scene loads the saved heights.
        let restarted = Scene::new(RegionInfo::new("T", 1000, 1000), SceneConfig::default());
        let saved = load_saved_terrain(&store, region).await.expect("saved terrain");
        restarted.set_terrain(&saved);
        assert!((restarted.terrain_height_at(100.0, 100.0) - edited).abs() < 0.001);
        assert_eq!(restarted.terrain_height_at(10.0, 10.0), 21.0);
        let _ = std::fs::remove_file(&db_path);
    }
}
