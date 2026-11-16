// SPDX-License-Identifier: LGPL-3.0-or-later

use glam::{Quat, Vec3};
use uuid::Uuid;
use welkin_data::simulation::{PrimRow, PrimShapeRow};
use welkin_msg::welkin_types::PermissionMask;
use welkin_scene::group::SceneObjectGroup;
use welkin_scene::part::{PartColor, PrimFlags, SceneObjectPart};

pub fn part_from_rows(row: &PrimRow, shape: Option<&PrimShapeRow>) -> SceneObjectPart {
    let owner = Uuid::parse_str(&row.owner_id).unwrap_or(Uuid::nil());
    let mut part = SceneObjectPart::new(&row.name, owner);

    part.local_id = 0;
    part.uuid = Uuid::parse_str(&row.uuid).unwrap_or_else(|_| Uuid::new_v4());

    part.name = row.name.clone();
    part.text = row.text.clone();
    part.description = row.description.clone();
    part.sit_name = row.sit_name.clone();
    part.touch_name = row.touch_name.clone();

    part.flags = PrimFlags::from_bits_truncate(row.object_flags as u32);

    part.creator_id = Uuid::parse_str(&row.creator_id).unwrap_or(Uuid::nil());
    part.owner_id = Uuid::parse_str(&row.owner_id).unwrap_or(Uuid::nil());
    part.group_id = Uuid::parse_str(&row.group_id).unwrap_or(Uuid::nil());
    part.last_owner_id = Uuid::parse_str(&row.last_owner_id).unwrap_or(Uuid::nil());
    part.rezzer_id = Uuid::parse_str(&row.rezzer_id).unwrap_or(Uuid::nil());

    part.owner_mask = PermissionMask::from_bits_truncate(row.owner_mask as u32);
    part.next_owner_mask = PermissionMask::from_bits_truncate(row.next_owner_mask as u32);
    part.group_mask = PermissionMask::from_bits_truncate(row.group_mask as u32);
    part.everyone_mask = PermissionMask::from_bits_truncate(row.everyone_mask as u32);
    part.base_mask = PermissionMask::from_bits_truncate(row.base_mask as u32);

    part.offset_position = Vec3::new(row.position_x as f32, row.position_y as f32, row.position_z as f32);
    part.group_position = Vec3::new(row.group_position_x as f32, row.group_position_y as f32, row.group_position_z as f32);
    part.velocity = Vec3::new(row.velocity_x as f32, row.velocity_y as f32, row.velocity_z as f32);
    part.angular_velocity = Vec3::new(row.angular_velocity_x as f32, row.angular_velocity_y as f32, row.angular_velocity_z as f32);
    part.acceleration = Vec3::new(row.acceleration_x as f32, row.acceleration_y as f32, row.acceleration_z as f32);

    let rotation = Quat::from_xyzw(row.rotation_x as f32, row.rotation_y as f32, row.rotation_z as f32, row.rotation_w as f32);
    part.rotation_offset = if rotation.length() == 0.0 {
        Quat::IDENTITY
    } else {
        rotation.normalize()
    };

    part.sit_target_position = Vec3::new(row.sit_target_offset_x as f32, row.sit_target_offset_y as f32, row.sit_target_offset_z as f32);

    let sit_target_orient = Quat::from_xyzw(
        row.sit_target_orient_x as f32,
        row.sit_target_orient_y as f32,
        row.sit_target_orient_z as f32,
        row.sit_target_orient_w as f32,
    );
    part.sit_target_orientation = if sit_target_orient.length() == 0.0 {
        Quat::IDENTITY
    } else {
        sit_target_orient.normalize()
    };

    part.pay_price = [row.pay_price, row.pay_button1, row.pay_button2, row.pay_button3, row.pay_button4];

    part.sound_id = Uuid::parse_str(&row.looped_sound).unwrap_or(Uuid::nil());
    part.sound_gain = row.looped_sound_gain as f32;
    if part.sound_id != Uuid::nil() {
        part.sound_flags = 1;
        part.sound_radius = 100.0;
    }
    part.texture_animation = row.texture_animation.clone();

    part.camera_eye_offset = Vec3::new(row.camera_eye_offset_x as f32, row.camera_eye_offset_y as f32, row.camera_eye_offset_z as f32);
    part.camera_at_offset = Vec3::new(row.camera_at_offset_x as f32, row.camera_at_offset_y as f32, row.camera_at_offset_z as f32);

    part.script_access_pin = row.script_access_pin;
    part.allowed_drop = row.allowed_drop != 0;
    part.die_at_edge = row.die_at_edge != 0;
    part.sale_price = row.sale_price;
    part.object_sale_type = row.sale_type as u8;

    part.text_color = PartColor::new(row.color_r as u8, row.color_g as u8, row.color_b as u8, row.color_a as u8);
    part.particle_system = row.particle_system.clone();
    part.click_action = row.click_action as u8;
    part.material = row.material as u8;

    part.collision_sound = Uuid::parse_str(&row.collision_sound).unwrap_or(Uuid::nil());
    part.collision_sound_volume = row.collision_sound_volume as f32;

    part.pass_touches = row.pass_touches != 0;
    part.pass_collisions = row.pass_collisions != 0;

    part.link_num = row.link_number;
    part.media_url = row.media_url.clone();

    part.physics_shape_type = row.physics_shape_type as u8;
    part.density = row.density as f32;
    part.gravity_modifier = row.gravity_modifier as f32;
    part.friction = row.friction as f32;
    part.bounce = row.restitution as f32;

    part.creation_date = row.creation_date;

    if let Some(s) = shape {
        part.scale = Vec3::new(s.scale_x as f32, s.scale_y as f32, s.scale_z as f32);

        part.shape.pcode = s.pcode as u8;
        part.shape.path_begin = s.path_begin as u16;
        part.shape.path_end = s.path_end as u16;
        part.shape.path_scale_x = s.path_scale_x as u8;
        part.shape.path_scale_y = s.path_scale_y as u8;
        part.shape.path_shear_x = s.path_shear_x as u8;
        part.shape.path_shear_y = s.path_shear_y as u8;
        part.shape.path_skew = s.path_skew as i8;
        part.shape.path_curve = s.path_curve as u8;
        part.shape.path_radius_offset = s.path_radius_offset as i8;
        part.shape.path_revolutions = s.path_revolutions as u8;
        part.shape.path_taper_x = s.path_taper_x as i8;
        part.shape.path_taper_y = s.path_taper_y as i8;
        part.shape.path_twist = s.path_twist as i8;
        part.shape.path_twist_begin = s.path_twist_begin as i8;

        part.shape.profile_begin = s.profile_begin as u16;
        part.shape.profile_end = s.profile_end as u16;
        part.shape.profile_curve = s.profile_curve as u8;
        part.shape.profile_hollow = s.profile_hollow as u16;

        part.shape.texture_entry = s.texture.clone();
        part.shape.extra_params = s.extra_params.clone();
        part.shape.state = s.state as u8;
        part.shape.last_attach_point = s.last_attach_point as u8;

        part.shape.media = s.media.clone();
        part.shape.mat_override = s.mat_ovrd.clone();
    }

    part
}

pub fn groups_from_rows(rows: Vec<(PrimRow, Option<PrimShapeRow>)>) -> Vec<SceneObjectGroup> {
    let mut group_order: Vec<String> = Vec::new();
    let mut grouped: Vec<Vec<(PrimRow, Option<PrimShapeRow>)>> = Vec::new();

    for item in rows {
        let key = item.0.scene_group_id.clone();
        if let Some(idx) = group_order.iter().position(|existing| existing == &key) {
            grouped[idx].push(item);
        } else {
            group_order.push(key);
            grouped.push(vec![item]);
        }
    }

    let mut result = Vec::new();
    for group_rows in grouped {
        if group_rows.is_empty() {
            continue;
        }

        if group_rows.iter().all(|(row, _)| Uuid::parse_str(&row.uuid).is_err()) {
            continue;
        }

        let key = group_rows[0].0.scene_group_id.clone();
        let root_idx = group_rows
            .iter()
            .position(|(row, _)| row.uuid == key)
            .unwrap_or(0);

        let mut parts: Vec<SceneObjectPart> = group_rows
            .iter()
            .map(|(row, shape)| part_from_rows(row, shape.as_ref()))
            .collect();

        let root_part = parts.remove(root_idx);
        let mut group = SceneObjectGroup::new(root_part);
        for child in parts {
            group.add_part(child);
        }
        result.push(group);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persist::{part_to_prim_row, shape_to_row};
    use glam::{Quat, Vec3};
    use uuid::Uuid;
    use welkin_scene::part::PartColor;

    #[test]
    fn roundtrip_part() {
        let owner = Uuid::nil();
        let mut part = SceneObjectPart::new("part-name", owner);

        part.uuid = Uuid::from_u128(0x0102_0304);
        part.creation_date = 1234;
        part.name = "stored-name".to_string();
        part.text = "text-value".to_string();
        part.description = "description-value".to_string();
        part.sit_name = "sit-name".to_string();
        part.touch_name = "touch-name".to_string();

        part.flags = PrimFlags::from_bits_truncate(0b10101);

        part.creator_id = Uuid::from_u128(1);
        part.owner_id = Uuid::from_u128(2);
        part.group_id = Uuid::from_u128(3);
        part.last_owner_id = Uuid::from_u128(4);
        part.rezzer_id = Uuid::from_u128(5);

        part.owner_mask = PermissionMask::from_bits_truncate(1);
        part.next_owner_mask = PermissionMask::from_bits_truncate(2);
        part.group_mask = PermissionMask::from_bits_truncate(3);
        part.everyone_mask = PermissionMask::from_bits_truncate(4);
        part.base_mask = PermissionMask::from_bits_truncate(5);

        part.offset_position = Vec3::new(1.0, 2.0, 3.0);
        part.group_position = Vec3::new(4.0, 5.0, 6.0);
        part.velocity = Vec3::new(7.0, 8.0, 9.0);
        part.angular_velocity = Vec3::new(10.0, 11.0, 12.0);
        part.acceleration = Vec3::new(13.0, 14.0, 15.0);

        part.rotation_offset = Quat::from_rotation_x(0.5);
        part.sit_target_position = Vec3::new(16.0, 17.0, 18.0);
        part.sit_target_orientation = Quat::from_rotation_y(0.5);

        part.pay_price = [10, 20, 30, 40, 50];

        part.sound_id = Uuid::from_u128(6);
        part.sound_gain = 0.25;
        part.texture_animation = vec![1, 2, 3];

        part.camera_eye_offset = Vec3::new(19.0, 20.0, 21.0);
        part.camera_at_offset = Vec3::new(22.0, 23.0, 24.0);

        part.script_access_pin = 99;
        part.allowed_drop = true;
        part.die_at_edge = true;
        part.sale_price = 77;
        part.object_sale_type = 2;

        part.text_color = PartColor::new(9, 8, 7, 6);
        part.particle_system = vec![4, 5, 6];
        part.click_action = 3;
        part.material = 4;

        part.collision_sound = Uuid::from_u128(7);
        part.collision_sound_volume = 0.75;

        part.pass_touches = true;
        part.pass_collisions = true;

        part.link_num = 42;
        part.media_url = "media-url".to_string();

        part.physics_shape_type = 1;
        part.density = 1.5;
        part.gravity_modifier = 2.5;
        part.friction = 0.5;
        part.bounce = 0.25;

        part.scale = Vec3::new(2.0, 3.0, 4.0);

        let shape = &mut part.shape;
        shape.pcode = 9;
        shape.state = 8;
        shape.last_attach_point = 7;
        shape.profile_curve = 6;
        shape.profile_begin = 1000;
        shape.profile_end = 2000;
        shape.profile_hollow = 3000;
        shape.path_curve = 5;
        shape.path_begin = 4000;
        shape.path_end = 5000;
        shape.path_scale_x = 4;
        shape.path_scale_y = 3;
        shape.path_shear_x = 2;
        shape.path_shear_y = 1;
        shape.path_twist = -2;
        shape.path_twist_begin = -3;
        shape.path_radius_offset = -4;
        shape.path_taper_x = -5;
        shape.path_taper_y = -6;
        shape.path_revolutions = 1;
        shape.path_skew = 7;
        shape.texture_entry = vec![1, 2];
        shape.extra_params = vec![3, 4];
        shape.media = "prim-media".to_string();
        shape.mat_override = vec![5, 6];

        let group_id = part.uuid.to_string();
        let row = part_to_prim_row(&part, "r", &group_id);
        let shape_row = shape_to_row(&part);
        let loaded = part_from_rows(&row, Some(&shape_row));

        assert_eq!(loaded.uuid, part.uuid);
        assert_eq!(loaded.creation_date, part.creation_date);
        assert_eq!(loaded.name, part.name);
        assert_eq!(loaded.text, part.text);
        assert_eq!(loaded.description, part.description);
        assert_eq!(loaded.sit_name, part.sit_name);
        assert_eq!(loaded.touch_name, part.touch_name);
        assert_eq!(loaded.flags.bits(), part.flags.bits());

        assert_eq!(loaded.creator_id, part.creator_id);
        assert_eq!(loaded.owner_id, part.owner_id);
        assert_eq!(loaded.group_id, part.group_id);
        assert_eq!(loaded.last_owner_id, part.last_owner_id);
        assert_eq!(loaded.rezzer_id, part.rezzer_id);

        assert_eq!(loaded.owner_mask.bits(), part.owner_mask.bits());
        assert_eq!(loaded.next_owner_mask.bits(), part.next_owner_mask.bits());
        assert_eq!(loaded.group_mask.bits(), part.group_mask.bits());
        assert_eq!(loaded.everyone_mask.bits(), part.everyone_mask.bits());
        assert_eq!(loaded.base_mask.bits(), part.base_mask.bits());

        assert_eq!(loaded.offset_position, part.offset_position);
        assert_eq!(loaded.group_position, part.group_position);
        assert_eq!(loaded.velocity, part.velocity);
        assert_eq!(loaded.angular_velocity, part.angular_velocity);
        assert_eq!(loaded.acceleration, part.acceleration);

        assert_eq!(loaded.rotation_offset, part.rotation_offset);
        assert_eq!(loaded.sit_target_position, part.sit_target_position);
        assert_eq!(loaded.sit_target_orientation, part.sit_target_orientation);

        assert_eq!(loaded.pay_price, part.pay_price);

        assert_eq!(loaded.sound_id, part.sound_id);
        assert_eq!(loaded.sound_gain, part.sound_gain);
        assert_eq!(loaded.texture_animation, part.texture_animation);

        assert_eq!(loaded.camera_eye_offset, part.camera_eye_offset);
        assert_eq!(loaded.camera_at_offset, part.camera_at_offset);

        assert_eq!(loaded.script_access_pin, part.script_access_pin);
        assert_eq!(loaded.allowed_drop, part.allowed_drop);
        assert_eq!(loaded.die_at_edge, part.die_at_edge);
        assert_eq!(loaded.sale_price, part.sale_price);
        assert_eq!(loaded.object_sale_type, part.object_sale_type);

        assert_eq!(loaded.text_color.0, part.text_color.0);
        assert_eq!(loaded.particle_system, part.particle_system);
        assert_eq!(loaded.click_action, part.click_action);
        assert_eq!(loaded.material, part.material);

        assert_eq!(loaded.collision_sound, part.collision_sound);
        assert_eq!(loaded.collision_sound_volume, part.collision_sound_volume);

        assert_eq!(loaded.pass_touches, part.pass_touches);
        assert_eq!(loaded.pass_collisions, part.pass_collisions);

        assert_eq!(loaded.link_num, part.link_num);
        assert_eq!(loaded.media_url, part.media_url);

        assert_eq!(loaded.physics_shape_type, part.physics_shape_type);
        assert_eq!(loaded.density, part.density);
        assert_eq!(loaded.gravity_modifier, part.gravity_modifier);
        assert_eq!(loaded.friction, part.friction);
        assert_eq!(loaded.bounce, part.bounce);

        assert_eq!(loaded.scale, part.scale);

        assert_eq!(loaded.shape.pcode, part.shape.pcode);
        assert_eq!(loaded.shape.path_begin, part.shape.path_begin);
        assert_eq!(loaded.shape.path_end, part.shape.path_end);
        assert_eq!(loaded.shape.path_scale_x, part.shape.path_scale_x);
        assert_eq!(loaded.shape.path_scale_y, part.shape.path_scale_y);
        assert_eq!(loaded.shape.path_shear_x, part.shape.path_shear_x);
        assert_eq!(loaded.shape.path_shear_y, part.shape.path_shear_y);
        assert_eq!(loaded.shape.path_skew, part.shape.path_skew);
        assert_eq!(loaded.shape.path_curve, part.shape.path_curve);
        assert_eq!(loaded.shape.path_radius_offset, part.shape.path_radius_offset);
        assert_eq!(loaded.shape.path_revolutions, part.shape.path_revolutions);
        assert_eq!(loaded.shape.path_taper_x, part.shape.path_taper_x);
        assert_eq!(loaded.shape.path_taper_y, part.shape.path_taper_y);
        assert_eq!(loaded.shape.path_twist, part.shape.path_twist);
        assert_eq!(loaded.shape.path_twist_begin, part.shape.path_twist_begin);

        assert_eq!(loaded.shape.profile_begin, part.shape.profile_begin);
        assert_eq!(loaded.shape.profile_end, part.shape.profile_end);
        assert_eq!(loaded.shape.profile_curve, part.shape.profile_curve);
        assert_eq!(loaded.shape.profile_hollow, part.shape.profile_hollow);

        assert_eq!(loaded.shape.texture_entry, part.shape.texture_entry);
        assert_eq!(loaded.shape.extra_params, part.shape.extra_params);
        assert_eq!(loaded.shape.state, part.shape.state);
        assert_eq!(loaded.shape.last_attach_point, part.shape.last_attach_point);

        assert_eq!(loaded.shape.media, part.shape.media);
        assert_eq!(loaded.shape.mat_override, part.shape.mat_override);
    }

    #[test]
    fn groups_from_rows_counts() {
        let mut p1 = SceneObjectPart::new("root-a", Uuid::nil());
        p1.uuid = Uuid::from_u128(101);
        p1.link_num = 0;

        let mut p2 = SceneObjectPart::new("child-a", Uuid::nil());
        p2.uuid = Uuid::from_u128(102);
        p2.link_num = 1;

        let mut p3 = SceneObjectPart::new("root-b", Uuid::nil());
        p3.uuid = Uuid::from_u128(103);
        p3.link_num = 0;

        let rows = vec![
            (part_to_prim_row(&p1, "r", &p1.uuid.to_string()), None),
            (part_to_prim_row(&p2, "r", &p1.uuid.to_string()), None),
            (part_to_prim_row(&p3, "r", &p3.uuid.to_string()), None),
        ];

        let groups = groups_from_rows(rows);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].part_count(), 2);
        assert_eq!(groups[0].root_part().uuid, p1.uuid);
        assert_eq!(groups[1].part_count(), 1);
        assert_eq!(groups[1].root_part().uuid, p3.uuid);
    }

#[test]
fn a_looped_sound_comes_back_looping() {
    let mut part = SceneObjectPart::new("Loud", Uuid::new_v4());
    part.sound_id = Uuid::from_u128(0x77);
    part.sound_gain = 0.5;

    let row = part_to_prim_row(&part, "r", &part.uuid.to_string());
    let shape = shape_to_row(&part);
    let loaded = part_from_rows(&row, Some(&shape));

    assert_eq!(loaded.sound_id, Uuid::from_u128(0x77));
    assert_eq!(loaded.sound_gain, 0.5);
    assert_eq!(loaded.sound_flags, 1);
    assert_eq!(loaded.sound_radius, 100.0);
}

#[test]
fn no_sound_means_no_loop_flags() {
    let part = SceneObjectPart::new("Loud", Uuid::new_v4());

    let row = part_to_prim_row(&part, "r", &part.uuid.to_string());
    let shape = shape_to_row(&part);
    let loaded = part_from_rows(&row, Some(&shape));

    assert_eq!(loaded.sound_flags, 0);
    assert_eq!(loaded.sound_radius, 0.0);
}
}
