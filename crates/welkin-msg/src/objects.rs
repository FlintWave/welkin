// SPDX-License-Identifier: GPL-3.0-or-later

use aurora_msg::msgs::object_update;
use glam::{Quat, Vec3, Vec4};
use uuid::Uuid;

pub fn f32_to_u16(value: f32, lower: f32, upper: f32) -> u16 {
    ((value - lower) / (upper - lower) * 65535.0)
        .round()
        .clamp(0.0, 65535.0) as u16
}

pub fn pack_quat(q: Quat) -> Vec3 {
    let x = if q.w < 0.0 { -q.x } else { q.x };
    let y = if q.w < 0.0 { -q.y } else { q.y };
    let z = if q.w < 0.0 { -q.z } else { q.z };

    Vec3::new(x, y, z)
}

pub fn encode_terse(
    local_id: u32,
    state: u8,
    is_avatar: bool,
    foot_plane: Option<Vec4>,
    position: Vec3,
    velocity: Vec3,
    acceleration: Vec3,
    rotation: Quat,
    angular_velocity: Vec3,
) -> Vec<u8> {
    let mut buf = Vec::new();

    buf.extend_from_slice(&local_id.to_le_bytes());
    buf.push(state);
    buf.push(if is_avatar { 1u8 } else { 0u8 });

    if is_avatar {
        let foot_plane = foot_plane.unwrap_or(Vec4::ZERO);
        buf.extend_from_slice(&foot_plane.x.to_le_bytes());
        buf.extend_from_slice(&foot_plane.y.to_le_bytes());
        buf.extend_from_slice(&foot_plane.z.to_le_bytes());
        buf.extend_from_slice(&foot_plane.w.to_le_bytes());
    }

    buf.extend_from_slice(&position.x.to_le_bytes());
    buf.extend_from_slice(&position.y.to_le_bytes());
    buf.extend_from_slice(&position.z.to_le_bytes());

    buf.extend_from_slice(&f32_to_u16(velocity.x, -128.0, 128.0).to_le_bytes());
    buf.extend_from_slice(&f32_to_u16(velocity.y, -128.0, 128.0).to_le_bytes());
    buf.extend_from_slice(&f32_to_u16(velocity.z, -128.0, 128.0).to_le_bytes());

    buf.extend_from_slice(&f32_to_u16(acceleration.x, -64.0, 64.0).to_le_bytes());
    buf.extend_from_slice(&f32_to_u16(acceleration.y, -64.0, 64.0).to_le_bytes());
    buf.extend_from_slice(&f32_to_u16(acceleration.z, -64.0, 64.0).to_le_bytes());

    buf.extend_from_slice(&f32_to_u16(rotation.x, -1.0, 1.0).to_le_bytes());
    buf.extend_from_slice(&f32_to_u16(rotation.y, -1.0, 1.0).to_le_bytes());
    buf.extend_from_slice(&f32_to_u16(rotation.z, -1.0, 1.0).to_le_bytes());
    buf.extend_from_slice(&f32_to_u16(rotation.w, -1.0, 1.0).to_le_bytes());

    buf.extend_from_slice(&f32_to_u16(angular_velocity.x, -64.0, 64.0).to_le_bytes());
    buf.extend_from_slice(&f32_to_u16(angular_velocity.y, -64.0, 64.0).to_le_bytes());
    buf.extend_from_slice(&f32_to_u16(angular_velocity.z, -64.0, 64.0).to_le_bytes());

    buf
}

pub fn encode_object_data(
    position: Vec3,
    velocity: Vec3,
    acceleration: Vec3,
    rotation: Quat,
    angular_velocity: Vec3,
    foot_plane: Option<Vec4>,
) -> Vec<u8> {
    let mut buf = Vec::new();

    if let Some(foot_plane) = foot_plane {
        buf.extend_from_slice(&foot_plane.x.to_le_bytes());
        buf.extend_from_slice(&foot_plane.y.to_le_bytes());
        buf.extend_from_slice(&foot_plane.z.to_le_bytes());
        buf.extend_from_slice(&foot_plane.w.to_le_bytes());
    }

    buf.extend_from_slice(&position.x.to_le_bytes());
    buf.extend_from_slice(&position.y.to_le_bytes());
    buf.extend_from_slice(&position.z.to_le_bytes());

    buf.extend_from_slice(&velocity.x.to_le_bytes());
    buf.extend_from_slice(&velocity.y.to_le_bytes());
    buf.extend_from_slice(&velocity.z.to_le_bytes());

    buf.extend_from_slice(&acceleration.x.to_le_bytes());
    buf.extend_from_slice(&acceleration.y.to_le_bytes());
    buf.extend_from_slice(&acceleration.z.to_le_bytes());

    let packed_rotation = pack_quat(rotation);
    buf.extend_from_slice(&packed_rotation.x.to_le_bytes());
    buf.extend_from_slice(&packed_rotation.y.to_le_bytes());
    buf.extend_from_slice(&packed_rotation.z.to_le_bytes());

    buf.extend_from_slice(&angular_velocity.x.to_le_bytes());
    buf.extend_from_slice(&angular_velocity.y.to_le_bytes());
    buf.extend_from_slice(&angular_velocity.z.to_le_bytes());

    buf
}

pub struct FullObjectUpdate {
    pub local_id: u32,
    pub full_id: Uuid,
    pub parent_id: u32,
    pub pcode: u8,
    pub state: u8,
    pub crc: u32,
    pub material: u8,
    pub click_action: u8,
    pub scale: Vec3,
    pub position: Vec3,
    pub velocity: Vec3,
    pub acceleration: Vec3,
    pub rotation: Quat,
    pub angular_velocity: Vec3,
    pub update_flags: u32,
    pub owner_id: Uuid,
    pub path_curve: u8,
    pub profile_curve: u8,
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
    pub profile_begin: u16,
    pub profile_end: u16,
    pub profile_hollow: u16,
    pub texture_entry: Vec<u8>,
    pub texture_anim: Vec<u8>,
    pub name_value: Vec<u8>,
    pub data: Vec<u8>,
    pub text: String,
    pub text_color: [u8; 4],
    pub media_url: String,
    pub ps_block: Vec<u8>,
    pub extra_params: Vec<u8>,
    pub sound_id: Uuid,
    pub sound_gain: f32,
    pub sound_flags: u8,
    pub sound_radius: f32,
    pub foot_plane: Option<Vec4>,
}

impl FullObjectUpdate {
    pub fn to_object_data(&self) -> object_update::ObjectData {
        object_update::ObjectData {
            id: self.local_id,
            state: self.state,
            full_id: self.full_id,
            crc: self.crc,
            p_code: self.pcode,
            material: self.material,
            click_action: self.click_action,
            scale: self.scale,
            object_data: encode_object_data(
                self.position,
                self.velocity,
                self.acceleration,
                self.rotation,
                self.angular_velocity,
                self.foot_plane,
            ),
            parent_id: self.parent_id,
            update_flags: self.update_flags,
            path_curve: self.path_curve,
            profile_curve: self.profile_curve,
            path_begin: self.path_begin,
            path_end: self.path_end,
            path_scale_x: self.path_scale_x,
            path_scale_y: self.path_scale_y,
            path_shear_x: self.path_shear_x,
            path_shear_y: self.path_shear_y,
            path_twist: self.path_twist,
            path_twist_begin: self.path_twist_begin,
            path_radius_offset: self.path_radius_offset,
            path_taper_x: self.path_taper_x,
            path_taper_y: self.path_taper_y,
            path_revolutions: self.path_revolutions,
            path_skew: self.path_skew,
            profile_begin: self.profile_begin,
            profile_end: self.profile_end,
            profile_hollow: self.profile_hollow,
            texture_entry: self.texture_entry.clone(),
            texture_anim: self.texture_anim.clone(),
            name_value: self.name_value.clone(),
            data: self.data.clone(),
            text: self.text.as_bytes().to_vec(),
            text_color: vec![
                self.text_color[0],
                self.text_color[1],
                self.text_color[2],
                255 - self.text_color[3],
            ],
            media_url: self.media_url.as_bytes().to_vec(),
            ps_block: self.ps_block.clone(),
            extra_params: self.extra_params.clone(),
            sound: self.sound_id,
            owner_id: self.owner_id,
            gain: self.sound_gain,
            flags: self.sound_flags,
            radius: self.sound_radius,
            joint_type: 0,
            joint_pivot: Vec3::ZERO,
            joint_axis_or_anchor: Vec3::ZERO,
        }
    }
}

pub fn encode_compressed(update: &FullObjectUpdate) -> (Vec<u8>, u32) {
    let mut flags: u32 = 0;

    if update.angular_velocity != Vec3::ZERO {
        flags |= 0x80;
    }
    if update.parent_id != 0 {
        flags |= 0x20;
    }
    if !update.text.is_empty() {
        flags |= 0x04;
    }
    if !update.media_url.is_empty() {
        flags |= 0x200;
    }
    if update.ps_block.len() == 86 {
        flags |= 0x08;
    }
    if !update.sound_id.is_nil() {
        flags |= 0x10;
    }
    if !update.name_value.is_empty() {
        flags |= 0x100;
    }
    if !update.texture_anim.is_empty() {
        flags |= 0x40;
    }

    let mut data = Vec::new();

    data.extend_from_slice(update.full_id.as_bytes());
    data.extend_from_slice(&update.local_id.to_le_bytes());
    data.push(update.pcode);
    data.push(update.state);
    data.extend_from_slice(&update.crc.to_le_bytes());
    data.push(update.material);
    data.push(update.click_action);
    data.extend_from_slice(&update.scale.x.to_le_bytes());
    data.extend_from_slice(&update.scale.y.to_le_bytes());
    data.extend_from_slice(&update.scale.z.to_le_bytes());
    data.extend_from_slice(&update.position.x.to_le_bytes());
    data.extend_from_slice(&update.position.y.to_le_bytes());
    data.extend_from_slice(&update.position.z.to_le_bytes());

    let packed = pack_quat(update.rotation);
    data.extend_from_slice(&packed.x.to_le_bytes());
    data.extend_from_slice(&packed.y.to_le_bytes());
    data.extend_from_slice(&packed.z.to_le_bytes());

    data.extend_from_slice(&flags.to_le_bytes());
    data.extend_from_slice(update.owner_id.as_bytes());

    if update.angular_velocity != Vec3::ZERO {
        data.extend_from_slice(&update.angular_velocity.x.to_le_bytes());
        data.extend_from_slice(&update.angular_velocity.y.to_le_bytes());
        data.extend_from_slice(&update.angular_velocity.z.to_le_bytes());
    }
    if update.parent_id != 0 {
        data.extend_from_slice(&update.parent_id.to_le_bytes());
    }
    if !update.text.is_empty() {
        data.extend_from_slice(update.text.as_bytes());
        data.push(0);
        data.push(update.text_color[0]);
        data.push(update.text_color[1]);
        data.push(update.text_color[2]);
        data.push(255 - update.text_color[3]);
    }
    if !update.media_url.is_empty() {
        data.extend_from_slice(update.media_url.as_bytes());
        data.push(0);
    }
    if update.ps_block.len() == 86 {
        data.extend_from_slice(&update.ps_block);
    }

    if update.extra_params.is_empty() {
        data.push(0);
    } else {
        data.extend_from_slice(&update.extra_params);
    }

    if !update.sound_id.is_nil() {
        data.extend_from_slice(update.sound_id.as_bytes());
        data.extend_from_slice(&update.sound_gain.to_le_bytes());
        data.push(update.sound_flags);
        data.extend_from_slice(&update.sound_radius.to_le_bytes());
    }
    if !update.name_value.is_empty() {
        data.extend_from_slice(&update.name_value);
        data.push(0);
    }

    data.push(update.path_curve);
    data.extend_from_slice(&update.path_begin.to_le_bytes());
    data.extend_from_slice(&update.path_end.to_le_bytes());
    data.push(update.path_scale_x);
    data.push(update.path_scale_y);
    data.push(update.path_shear_x);
    data.push(update.path_shear_y);
    data.push(update.path_twist as u8);
    data.push(update.path_twist_begin as u8);
    data.push(update.path_radius_offset as u8);
    data.push(update.path_taper_x as u8);
    data.push(update.path_taper_y as u8);
    data.push(update.path_revolutions);
    data.push(update.path_skew as u8);
    data.push(update.profile_curve);
    data.extend_from_slice(&update.profile_begin.to_le_bytes());
    data.extend_from_slice(&update.profile_end.to_le_bytes());
    data.extend_from_slice(&update.profile_hollow.to_le_bytes());

    data.extend_from_slice(&(update.texture_entry.len() as u32).to_le_bytes());
    data.extend_from_slice(&update.texture_entry);

    if !update.texture_anim.is_empty() {
        data.extend_from_slice(&(update.texture_anim.len() as u32).to_le_bytes());
        data.extend_from_slice(&update.texture_anim);
    }

    (data, flags)
}

#[cfg(test)]
mod tests {
    use super::*;

    use aurora_msg::msgs::improved_terse_object_update;
    use aurora_msg::unpack_quat;
    use glam::{Quat, Vec3, Vec4};

    fn u16_to_f32(v: u16, lower: f32, upper: f32) -> f32 {
        let delta = upper - lower;
        let mut val = v as f32 / 65535.0 * delta + lower;
        let max_error = delta / 65535.0;
        if val.abs() < max_error {
            val = 0.0;
        }
        val
    }

    #[test]
    fn test_f32_u16_roundtrip() {
        for &value in &[-128.0f32, -100.0, -10.0, -1.0, 0.0, 1.0, 10.0, 100.0, 128.0] {
            let encoded = f32_to_u16(value, -128.0, 128.0);
            let decoded = u16_to_f32(encoded, -128.0, 128.0);
            assert!(
                (decoded - value).abs() <= 0.01,
                "value={} decoded={}",
                value,
                decoded
            );
        }
    }

    #[test]
    fn test_pack_quat_roundtrip() {
        let q = Quat::from_xyzw(0.1, 0.2, 0.3, 0.9).normalize();
        let packed = pack_quat(q);
        let unpacked = unpack_quat(packed);

        assert!((unpacked.x - q.x).abs() <= 0.001);
        assert!((unpacked.y - q.y).abs() <= 0.001);
        assert!((unpacked.z - q.z).abs() <= 0.001);
        assert!((unpacked.w - q.w).abs() <= 0.001);
    }

    #[test]
    fn test_encode_terse_roundtrip() {
        let data = encode_terse(
            42,
            3,
            false,
            None,
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::new(0.5, -0.25, 0.75),
            Vec3::new(0.1, -0.1, 0.2),
            Quat::IDENTITY,
            Vec3::ZERO,
        );
        let block = improved_terse_object_update::ObjectData {
            data,
            texture_entry: vec![],
        };
        let parsed = aurora_net::objects::parse_terse(&block).expect("parse_terse failed");
        assert_eq!(parsed.local_id, 42);
        assert_eq!(parsed.state, 3);
        assert!(!parsed.is_avatar);
        assert!((parsed.position.x - 1.0).abs() < 0.001);
        assert!((parsed.position.y - 2.0).abs() < 0.001);
        assert!((parsed.position.z - 3.0).abs() < 0.001);
        assert!((parsed.velocity.x - 0.5).abs() < 0.01);
        assert!((parsed.velocity.y + 0.25).abs() < 0.01);
        assert!(parsed.angular_velocity.x.abs() < 0.01);
    }

    #[test]
    fn test_full_object_roundtrip() {
        use uuid::Uuid;

        let full = FullObjectUpdate {
            local_id: 42,
            full_id: Uuid::from_u128(0x12345678),
            parent_id: 0,
            pcode: 47,
            state: 5,
            crc: 0,
            material: 0,
            click_action: 0,
            scale: Vec3::new(1.0, 2.0, 3.0),
            position: Vec3::new(10.0, 20.0, 30.0),
            velocity: Vec3::ZERO,
            acceleration: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            angular_velocity: Vec3::ZERO,
            update_flags: 0,
            owner_id: Uuid::from_u128(0xABCDEF),
            path_curve: 16,
            profile_curve: 1,
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
            profile_begin: 0,
            profile_end: 0,
            profile_hollow: 0,
            texture_entry: vec![],
            texture_anim: vec![],
            name_value: vec![],
            data: vec![],
            text: String::new(),
            text_color: [0, 0, 0, 0],
            media_url: String::new(),
            ps_block: vec![],
            extra_params: vec![],
            sound_id: Uuid::nil(),
            sound_gain: 0.0,
            sound_flags: 0,
            sound_radius: 0.0,
            foot_plane: None,
        };

        let object_data = full.to_object_data();
        let parsed = aurora_net::objects::parse_full(&object_data).expect("parse_full failed");

        assert_eq!(parsed.local_id, 42);
        assert_eq!(parsed.full_id, Uuid::from_u128(0x12345678));
        assert_eq!(parsed.pcode, 47);
        assert_eq!(parsed.state, 5);
        assert!((parsed.position.x - 10.0).abs() < 0.001);
        assert!((parsed.position.y - 20.0).abs() < 0.001);
        assert!((parsed.position.z - 30.0).abs() < 0.001);
        assert!((parsed.scale.x - 1.0).abs() < 0.001);
        assert!((parsed.scale.y - 2.0).abs() < 0.001);
        assert!((parsed.scale.z - 3.0).abs() < 0.001);
        assert_eq!(parsed.owner_id, Uuid::from_u128(0xABCDEF));
    }

    #[test]
    fn test_encode_compressed_roundtrip() {
        use uuid::Uuid;

        let update = FullObjectUpdate {
            local_id: 99,
            full_id: Uuid::from_u128(0xDEADBEEF),
            parent_id: 0,
            pcode: 47,
            state: 0,
            crc: 12345,
            material: 3,
            click_action: 0,
            scale: Vec3::new(2.0, 2.0, 2.0),
            position: Vec3::new(50.0, 60.0, 70.0),
            velocity: Vec3::ZERO,
            acceleration: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            angular_velocity: Vec3::ZERO,
            update_flags: 0,
            owner_id: Uuid::from_u128(0xCAFE),
            path_curve: 16,
            profile_curve: 1,
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
            profile_begin: 0,
            profile_end: 0,
            profile_hollow: 0,
            texture_entry: vec![],
            texture_anim: vec![],
            name_value: vec![],
            data: vec![],
            text: String::new(),
            text_color: [0, 0, 0, 0],
            media_url: String::new(),
            ps_block: vec![],
            extra_params: vec![],
            sound_id: Uuid::nil(),
            sound_gain: 0.0,
            sound_flags: 0,
            sound_radius: 0.0,
            foot_plane: None,
        };

        let (data, _compressed_flags) = encode_compressed(&update);
        let parsed = aurora_net::objects::parse_compressed(&data, update.update_flags)
            .expect("parse_compressed failed");

        assert_eq!(parsed.local_id, 99);
        assert_eq!(parsed.full_id, Uuid::from_u128(0xDEADBEEF));
        assert_eq!(parsed.pcode, 47);
        assert_eq!(parsed.crc, 12345);
        assert_eq!(parsed.material, 3);
        assert!((parsed.position.x - 50.0).abs() < 0.001);
        assert!((parsed.position.y - 60.0).abs() < 0.001);
        assert!((parsed.position.z - 70.0).abs() < 0.001);
        assert!((parsed.scale.x - 2.0).abs() < 0.001);
        assert!((parsed.scale.y - 2.0).abs() < 0.001);
        assert!((parsed.scale.z - 2.0).abs() < 0.001);
        assert_eq!(parsed.owner_id, Uuid::from_u128(0xCAFE));
    }

    #[test]
    fn test_encode_object_data_sizes() {
        let non_avatar = encode_object_data(
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::ZERO,
            Quat::IDENTITY,
            Vec3::ZERO,
            None,
        );
        assert_eq!(non_avatar.len(), 60);

        let avatar = encode_object_data(
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::ZERO,
            Quat::IDENTITY,
            Vec3::ZERO,
            Some(Vec4::ZERO),
        );
        assert_eq!(avatar.len(), 76);
    }
}
