// SPDX-License-Identifier: GPL-3.0-or-later

use glam::{Quat, Vec3};
use uuid::Uuid;
use welkin_types::PermissionMask;

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct PrimFlags: u32 {
        const NONE                  = 0;
        const PHYSICS               = 0x0000_0001;
        const CREATE_SELECTED       = 0x0000_0002;
        const OBJECT_MODIFY         = 0x0000_0004;
        const OBJECT_COPY           = 0x0000_0008;
        const OBJECT_ANY_OWNER      = 0x0000_0010;
        const OBJECT_YOU_OWNER      = 0x0000_0020;
        const SCRIPTED              = 0x0000_0040;
        const TOUCH                 = 0x0000_0080;
        const OBJECT_MOVE           = 0x0000_0100;
        const TAKES_MONEY           = 0x0000_0200;
        const PHANTOM               = 0x0000_0400;
        const INVENTORY_EMPTY       = 0x0000_0800;
        const AFFECTS_NAVMESH       = 0x0000_1000;
        const CHARACTER             = 0x0000_2000;
        const VOLUME_DETECT         = 0x0000_4000;
        const INCLUDE_IN_SEARCH     = 0x0000_8000;
        const ALLOW_INVENTORY_DROP  = 0x0001_0000;
        const OBJECT_TRANSFER       = 0x0002_0000;
        const OBJECT_GROUP_OWNED    = 0x0004_0000;
        const OBJECT_YOU_OFFICER    = 0x0008_0000;
        const CAMERA_DECOUPLED      = 0x0010_0000;
        const ANIM_SOURCE           = 0x0020_0000;
        const CAMERA_SOURCE         = 0x0040_0000;
        const CAST_SHADOWS          = 0x0100_0000;
        const OBJECT_OWNER_MODIFY   = 0x1000_0000;
        const TEMPORARY_ON_REZ      = 0x2000_0000;
        const TEMPORARY             = 0x4000_0000;
        const ZONE_FLY              = 0x8000_0000;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PartColor(pub [u8; 4]);

impl PartColor {
    pub const WHITE: Self = Self([255, 255, 255, 255]);

    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self([r, g, b, a])
    }

    pub fn r(&self) -> u8 {
        self.0[0]
    }
    pub fn g(&self) -> u8 {
        self.0[1]
    }
    pub fn b(&self) -> u8 {
        self.0[2]
    }
    pub fn a(&self) -> u8 {
        self.0[3]
    }
}

impl Default for PartColor {
    fn default() -> Self {
        Self::WHITE
    }
}

#[derive(Debug, Clone)]
pub struct PrimShape {
    pub pcode: u8,
    pub state: u8,
    pub last_attach_point: u8,
    pub profile_curve: u8,
    pub profile_begin: u16,
    pub profile_end: u16,
    pub profile_hollow: u16,
    pub path_curve: u8,
    pub path_begin: u16,
    pub path_end: u16,
    pub path_scale_x: u8,
    pub path_scale_y: u8,
    pub path_shear_x: u8,
    pub path_shear_y: u8,
    pub path_twist: i8,
    pub path_twist_begin: i8,
    pub path_radius_offset: i8,
    pub path_taper_x: i8,
    pub path_taper_y: i8,
    pub path_revolutions: u8,
    pub path_skew: i8,
    pub sculpt_texture: Uuid,
    pub sculpt_type: u8,
    pub texture_entry: Vec<u8>,
    pub extra_params: Vec<u8>,
    pub flexi_entry: bool,
    pub flexi_softness: i32,
    pub flexi_tension: f32,
    pub flexi_drag: f32,
    pub flexi_gravity: f32,
    pub flexi_wind: f32,
    pub flexi_force_x: f32,
    pub flexi_force_y: f32,
    pub flexi_force_z: f32,
    pub light_entry: bool,
    pub light_color_r: f32,
    pub light_color_g: f32,
    pub light_color_b: f32,
    pub light_color_a: f32,
    pub light_radius: f32,
    pub light_cutoff: f32,
    pub light_falloff: f32,
    pub light_intensity: f32,
    pub sculpt_entry: bool,
    pub profile_shape: String,
    pub hollow_shape: String,
    pub media: String,
    pub mat_override: Vec<u8>,
}

impl Default for PrimShape {
    fn default() -> Self {
        Self {
            pcode: 9, // LL_PCODE_VOLUME
            state: 0,
            last_attach_point: 0,
            profile_curve: 1, // circle
            profile_begin: 0,
            profile_end: 0,
            profile_hollow: 0,
            path_curve: 16, // line
            path_begin: 0,
            path_end: 0,
            path_scale_x: 100,
            path_scale_y: 100,
            path_shear_x: 0,
            path_shear_y: 0,
            path_twist: 0,
            path_twist_begin: 0,
            path_radius_offset: 0,
            path_taper_x: 0,
            path_taper_y: 0,
            path_revolutions: 0,
            path_skew: 0,
            sculpt_texture: Uuid::nil(),
            sculpt_type: 0,
            texture_entry: Vec::new(),
            extra_params: Vec::new(),
            flexi_entry: false,
            flexi_softness: 0,
            flexi_tension: 0.0,
            flexi_drag: 0.0,
            flexi_gravity: 0.0,
            flexi_wind: 0.0,
            flexi_force_x: 0.0,
            flexi_force_y: 0.0,
            flexi_force_z: 0.0,
            light_entry: false,
            light_color_r: 0.0,
            light_color_g: 0.0,
            light_color_b: 0.0,
            light_color_a: 1.0,
            light_radius: 0.0,
            light_cutoff: 0.0,
            light_falloff: 0.0,
            light_intensity: 0.0,
            sculpt_entry: false,
            profile_shape: String::from("Square"),
            hollow_shape: String::from("Same"),
            media: String::new(),
            mat_override: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SceneObjectPart {
    // Identity
    pub uuid: Uuid,
    pub local_id: u32,
    pub name: String,
    pub description: String,
    pub creator_id: Uuid,
    pub creator_data: String,
    pub owner_id: Uuid,
    pub last_owner_id: Uuid,
    pub group_id: Uuid,
    pub rezzer_id: Uuid,
    pub folder_id: Uuid,

    // Geometry
    pub group_position: Vec3,
    pub offset_position: Vec3,
    pub rotation_offset: Quat,
    pub scale: Vec3,
    pub velocity: Vec3,
    pub angular_velocity: Vec3,
    pub acceleration: Vec3,

    // Shape
    pub shape: PrimShape,

    // Physics
    pub physics_shape_type: u8,
    pub density: f32,
    pub friction: f32,
    pub bounce: f32,
    pub gravity_modifier: f32,
    pub force: Vec3,
    pub torque: Vec3,
    pub buoyancy: f32,
    pub volume_detect_active: bool,

    // Permissions
    pub base_mask: PermissionMask,
    pub owner_mask: PermissionMask,
    pub group_mask: PermissionMask,
    pub everyone_mask: PermissionMask,
    pub next_owner_mask: PermissionMask,

    // Visual
    pub text: String,
    pub text_color: PartColor,
    pub material: u8,
    pub click_action: u8,
    pub texture_animation: Vec<u8>,
    pub particle_system: Vec<u8>,
    pub media_url: String,

    // Sound
    pub sound_id: Uuid,
    pub sound_gain: f32,
    pub sound_flags: u8,
    pub sound_radius: f32,
    pub sound_queueing: bool,
    pub collision_sound: Uuid,
    pub collision_sound_volume: f32,

    // Sit
    pub sit_name: String,
    pub touch_name: String,
    pub sit_target_position: Vec3,
    pub sit_target_orientation: Quat,
    pub sit_target_position_ll: Vec3,
    pub sit_target_orientation_ll: Quat,
    pub stand_target: Vec3,
    pub camera_eye_offset: Vec3,
    pub camera_at_offset: Vec3,
    pub sit_act_range: f32,

    // Links and hierarchy
    pub link_num: i32,
    pub parent_id: u32,

    // Flags
    pub flags: PrimFlags,
    pub allowed_drop: bool,
    pub die_at_edge: bool,
    pub return_at_edge: bool,
    pub pass_touches: bool,
    pub pass_collisions: bool,

    // Script
    pub script_access_pin: i32,

    // Commerce
    pub sale_price: i32,
    pub object_sale_type: u8,
    pub ownership_cost: i32,
    pub category: u32,

    // Time
    pub creation_date: i32,
    pub region_handle: u64,
    pub inventory_serial: u32,

    // Attachment
    pub attached_pos: Vec3,

    // Pay
    pub pay_price: [i32; 5],
}

impl Default for SceneObjectPart {
    fn default() -> Self {
        Self {
            uuid: Uuid::nil(),
            local_id: 0,
            name: String::new(),
            description: String::new(),
            creator_id: Uuid::nil(),
            creator_data: String::new(),
            owner_id: Uuid::nil(),
            last_owner_id: Uuid::nil(),
            group_id: Uuid::nil(),
            rezzer_id: Uuid::nil(),
            folder_id: Uuid::nil(),
            group_position: Vec3::ZERO,
            offset_position: Vec3::ZERO,
            rotation_offset: Quat::IDENTITY,
            scale: Vec3::new(0.5, 0.5, 0.5),
            velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            acceleration: Vec3::ZERO,
            shape: PrimShape::default(),
            physics_shape_type: 0,
            density: 1000.0,
            friction: 0.6,
            bounce: 0.5,
            gravity_modifier: 1.0,
            force: Vec3::ZERO,
            torque: Vec3::ZERO,
            buoyancy: 0.0,
            volume_detect_active: false,
            base_mask: PermissionMask::ALL,
            owner_mask: PermissionMask::ALL,
            group_mask: PermissionMask::NONE,
            everyone_mask: PermissionMask::NONE,
            next_owner_mask: PermissionMask::ALL,
            text: String::new(),
            text_color: PartColor::default(),
            material: 3, // Wood
            click_action: 0,
            texture_animation: Vec::new(),
            particle_system: Vec::new(),
            media_url: String::new(),
            sound_id: Uuid::nil(),
            sound_gain: 0.0,
            sound_flags: 0,
            sound_radius: 0.0,
            sound_queueing: false,
            collision_sound: Uuid::nil(),
            collision_sound_volume: 0.0,
            sit_name: String::new(),
            touch_name: String::new(),
            sit_target_position: Vec3::ZERO,
            sit_target_orientation: Quat::IDENTITY,
            sit_target_position_ll: Vec3::ZERO,
            sit_target_orientation_ll: Quat::IDENTITY,
            stand_target: Vec3::ZERO,
            camera_eye_offset: Vec3::ZERO,
            camera_at_offset: Vec3::ZERO,
            sit_act_range: 0.0,
            link_num: 0,
            parent_id: 0,
            flags: PrimFlags::NONE,
            allowed_drop: false,
            die_at_edge: false,
            return_at_edge: false,
            pass_touches: false,
            pass_collisions: false,
            script_access_pin: 0,
            sale_price: 10,
            object_sale_type: 0,
            ownership_cost: 0,
            category: 0,
            creation_date: 0,
            region_handle: 0,
            inventory_serial: 0,
            attached_pos: Vec3::ZERO,
            pay_price: [-2, -2, -2, -2, -2],
        }
    }
}

impl SceneObjectPart {
    pub fn new(name: &str, owner_id: Uuid) -> Self {
        Self {
            uuid: Uuid::new_v4(),
            name: name.to_string(),
            owner_id,
            creator_id: owner_id,
            last_owner_id: owner_id,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_part_has_sane_values() {
        let part = SceneObjectPart::default();
        assert_eq!(part.density, 1000.0);
        assert_eq!(part.friction, 0.6);
        assert_eq!(part.bounce, 0.5);
        assert_eq!(part.gravity_modifier, 1.0);
        assert_eq!(part.rotation_offset, Quat::IDENTITY);
        assert_eq!(part.base_mask, PermissionMask::ALL);
        assert_eq!(part.scale, Vec3::new(0.5, 0.5, 0.5));
        assert_eq!(part.shape.pcode, 9);
    }

    #[test]
    fn new_part_has_uuid_and_owner() {
        let owner = Uuid::new_v4();
        let part = SceneObjectPart::new("Test Prim", owner);
        assert!(!part.uuid.is_nil());
        assert_eq!(part.owner_id, owner);
        assert_eq!(part.creator_id, owner);
        assert_eq!(part.name, "Test Prim");
    }

    #[test]
    fn prim_flags_values() {
        assert_eq!(PrimFlags::PHYSICS.bits(), 0x01);
        assert_eq!(PrimFlags::PHANTOM.bits(), 0x0400);
        assert_eq!(PrimFlags::TEMPORARY_ON_REZ.bits(), 0x2000_0000);
        let combined = PrimFlags::PHYSICS | PrimFlags::PHANTOM;
        assert!(combined.contains(PrimFlags::PHYSICS));
        assert!(combined.contains(PrimFlags::PHANTOM));
    }

    #[test]
    fn part_color_accessors() {
        let c = PartColor::new(10, 20, 30, 40);
        assert_eq!(c.r(), 10);
        assert_eq!(c.g(), 20);
        assert_eq!(c.b(), 30);
        assert_eq!(c.a(), 40);
    }
}
