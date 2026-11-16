// SPDX-License-Identifier: LGPL-3.0-or-later

//! Build a scene part from an `ObjectAdd` packet.
//!
//! Mirrors OpenSim's `LLClientView.GetShapeFromAddPacket`: every path/profile
//! field the viewer sends is copied into the part's shape, together with the
//! prim code, state and scale. (OpenSim also copies `State` into
//! `LastAttachPoint`, which is reproduced here.) The texture entry is left
//! empty so the default plywood texture is used when updates are built.

use uuid::Uuid;
use welkin_msg::msgs::ObjectAdd;
use welkin_scene::part::SceneObjectPart;

/// Create the root part for an `ObjectAdd`, positioned at the viewer's ray end.
pub fn part_from_object_add(add: &ObjectAdd) -> SceneObjectPart {
    let d = &add.object_data;
    let mut part = SceneObjectPart::default();
    part.uuid = Uuid::new_v4();
    part.owner_id = add.agent_data.agent_id;
    part.group_position = d.ray_end;
    part.scale = d.scale;
    part.rotation_offset = d.rotation;
    part.name = "Object".to_string();

    let shape = &mut part.shape;
    shape.pcode = d.p_code;
    shape.state = d.state;
    shape.last_attach_point = d.state;
    shape.path_curve = d.path_curve;
    shape.profile_curve = d.profile_curve;
    shape.path_begin = d.path_begin;
    shape.path_end = d.path_end;
    shape.path_scale_x = d.path_scale_x;
    shape.path_scale_y = d.path_scale_y;
    shape.path_shear_x = d.path_shear_x;
    shape.path_shear_y = d.path_shear_y;
    shape.path_twist = d.path_twist;
    shape.path_twist_begin = d.path_twist_begin;
    shape.path_radius_offset = d.path_radius_offset;
    shape.path_taper_x = d.path_taper_x;
    shape.path_taper_y = d.path_taper_y;
    shape.path_revolutions = d.path_revolutions;
    shape.path_skew = d.path_skew;
    shape.profile_begin = d.profile_begin;
    shape.profile_end = d.profile_end;
    shape.profile_hollow = d.profile_hollow;
    part
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{Quat, Vec3};

    #[test]
    fn tube_shape_is_applied() {
        let mut add = ObjectAdd::default();
        add.agent_data.agent_id = Uuid::new_v4();
        let d = &mut add.object_data;
        d.p_code = 9;
        d.path_curve = 32; // circle path
        d.profile_curve = 0; // circle profile
        d.profile_hollow = 95;
        d.path_begin = 100;
        d.path_end = 200;
        d.path_scale_x = 150;
        d.path_scale_y = 90;
        d.path_shear_x = 3;
        d.path_shear_y = 4;
        d.path_twist = -12;
        d.path_twist_begin = 7;
        d.path_radius_offset = -5;
        d.path_taper_x = 20;
        d.path_taper_y = -20;
        d.path_revolutions = 55;
        d.path_skew = -9;
        d.profile_begin = 1000;
        d.profile_end = 2000;
        d.state = 3;
        d.scale = Vec3::new(2.0, 3.0, 4.0);
        d.rotation = Quat::from_rotation_z(0.5);
        d.ray_end = Vec3::new(10.0, 20.0, 30.0);

        let part = part_from_object_add(&add);
        let s = &part.shape;
        assert_eq!(s.pcode, 9);
        assert_eq!(s.path_curve, 32);
        assert_eq!(s.profile_curve, 0);
        assert_eq!(s.profile_hollow, 95);
        assert_eq!((s.path_begin, s.path_end), (100, 200));
        assert_eq!((s.path_scale_x, s.path_scale_y), (150, 90));
        assert_eq!((s.path_shear_x, s.path_shear_y), (3, 4));
        assert_eq!((s.path_twist, s.path_twist_begin), (-12, 7));
        assert_eq!(s.path_radius_offset, -5);
        assert_eq!((s.path_taper_x, s.path_taper_y), (20, -20));
        assert_eq!(s.path_revolutions, 55);
        assert_eq!(s.path_skew, -9);
        assert_eq!((s.profile_begin, s.profile_end), (1000, 2000));
        assert_eq!(s.state, 3);
        assert_eq!(s.last_attach_point, 3, "OpenSim copies State into LastAttachPoint");
        assert_eq!(part.scale, Vec3::new(2.0, 3.0, 4.0));
        assert_eq!(part.group_position, Vec3::new(10.0, 20.0, 30.0));
        assert_eq!(part.owner_id, add.agent_data.agent_id);
    }

    #[test]
    fn shape_reaches_the_viewer_update() {
        let mut add = ObjectAdd::default();
        add.object_data.path_curve = 32;
        add.object_data.profile_curve = 0;
        add.object_data.profile_hollow = 95;
        let part = part_from_object_add(&add);
        let upd = crate::scene_sync::full_update_for_part(&part, 0, true);
        assert_eq!(upd.path_curve, 32);
        assert_eq!(upd.profile_curve, 0);
        assert_eq!(upd.profile_hollow, 95);
    }
}
