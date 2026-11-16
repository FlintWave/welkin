// SPDX-License-Identifier: LGPL-3.0-or-later

use uuid::Uuid;
use welkin_msg::{Msg, msgs};

pub const PARCEL_FLAGS_DEFAULT: u32 =
    (1 << 0) | (1 << 3) | (1 << 6) | (1 << 1) | (1 << 13) | (1 << 26) | (1 << 28);

pub const SEQ_AGENT_PARCEL: i32 = -10000;

pub struct RegionParcel {
    pub region_size: u32,
    pub owner_id: Uuid,
    pub name: String,
    pub description: String,
    pub flags: u32,
    pub max_prims: i32,
}

impl RegionParcel {
    pub fn new(region_size: u32, owner_id: Uuid) -> Self {
        Self {
            region_size,
            owner_id,
            name: "Your Parcel".to_string(),
            description: String::new(),
            flags: PARCEL_FLAGS_DEFAULT,
            max_prims: 15000,
        }
    }

    pub fn properties(
        &self,
        sequence_id: i32,
        snap_selection: bool,
        total_prims: i32,
        agent: Uuid,
    ) -> msgs::ParcelProperties {
        let owner_prims = if agent == self.owner_id {
            total_prims
        } else {
            0
        };
        let other_prims = if agent != self.owner_id {
            total_prims
        } else {
            0
        };

        let mut name = self.name.clone().into_bytes();
        name.push(b'\0');
        let mut desc = self.description.clone().into_bytes();
        desc.push(b'\0');

        msgs::ParcelProperties {
            parcel_data: msgs::parcel_properties::ParcelData {
                request_result: 0,
                sequence_id,
                snap_selection,
                self_count: 0,
                other_count: 0,
                public_count: 0,
                local_id: 1,
                owner_id: self.owner_id,
                is_group_owned: false,
                auction_id: 0,
                claim_date: 0,
                claim_price: 0,
                rent_price: 0,
                aabb_min: glam::Vec3::ZERO,
                aabb_max: glam::Vec3::new(
                    self.region_size as f32,
                    self.region_size as f32,
                    0.0,
                ),
                bitmap: vec![
                    0xFF;
                    ((self.region_size / 4)
                        * (self.region_size / 4)
                        / 8) as usize
                ],
                area: (self.region_size * self.region_size) as i32,
                status: 0,
                sim_wide_max_prims: self.max_prims,
                sim_wide_total_prims: total_prims,
                max_prims: self.max_prims,
                total_prims,
                owner_prims,
                group_prims: 0,
                other_prims,
                selected_prims: 0,
                parcel_prim_bonus: 1.0,
                other_clean_time: 0,
                parcel_flags: self.flags,
                sale_price: 0,
                name,
                desc,
                music_url: vec![0],
                media_url: vec![0],
                media_id: Uuid::nil(),
                media_auto_scale: 0,
                group_id: Uuid::nil(),
                pass_price: 0,
                pass_hours: 0.0,
                category: 0,
                auth_buyer_id: Uuid::nil(),
                snapshot_id: Uuid::nil(),
                user_location: glam::Vec3::ZERO,
                user_look_at: glam::Vec3::ZERO,
                landing_type: 1,
                region_push_override: false,
                region_deny_anonymous: false,
                region_deny_identified: false,
                region_deny_transacted: false,
            },
            age_verification_block: msgs::parcel_properties::AgeVerificationBlock {
                region_deny_age_unverified: false,
            },
            region_allow_access_block: msgs::parcel_properties::RegionAllowAccessBlock {
                region_allow_access_override: true,
            },
            parcel_environment_block: msgs::parcel_properties::ParcelEnvironmentBlock {
                parcel_environment_version: 0,
                region_allow_environment_override: false,
            },
        }
    }
}

pub fn is_parcel_request(id: welkin_msg::MsgId) -> bool {
    id == msgs::ParcelPropertiesRequest::ID
}

pub fn handle_parcel_request(
    parcel: &RegionParcel,
    agent: Uuid,
    body: &[u8],
    total_prims: i32,
) -> Option<msgs::ParcelProperties> {
    let req = msgs::ParcelPropertiesRequest::decode(body).ok()?;
    Some(parcel.properties(
        req.parcel_data.sequence_id,
        req.parcel_data.snap_selection,
        total_prims,
        agent,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;
    use welkin_msg::Writer;

    #[test]
    fn properties_fields() {
        let owner = Uuid::nil();
        let parcel = RegionParcel::new(256, owner);
        let props = parcel.properties(7, true, 42, owner);

        assert_eq!(props.parcel_data.local_id, 1);
        assert_eq!(props.parcel_data.area, 65536);
        assert_eq!(props.parcel_data.bitmap.len(), 512);
        assert!(props.parcel_data.bitmap.iter().all(|&b| b == 0xFF));
        assert!(props.parcel_data.parcel_flags & 1 == 1);
        assert_eq!(props.parcel_data.name, b"Your Parcel\0");
        assert_eq!(props.parcel_data.owner_prims, 42);
        assert_eq!(props.parcel_data.other_prims, 0);

        let other = Uuid::new_v4();
        let props = parcel.properties(7, true, 42, other);
        assert_eq!(props.parcel_data.owner_prims, 0);
        assert_eq!(props.parcel_data.other_prims, 42);
    }

    #[test]
    fn handle_request() {
        let owner = Uuid::nil();
        let parcel = RegionParcel::new(256, owner);
        let req = msgs::ParcelPropertiesRequest {
            agent_data: msgs::parcel_properties_request::AgentData {
                agent_id: owner,
                session_id: Uuid::nil(),
            },
            parcel_data: msgs::parcel_properties_request::ParcelData {
                sequence_id: 7,
                west: 0.0,
                south: 0.0,
                east: 256.0,
                north: 256.0,
                snap_selection: true,
            },
        };

        let mut w = Writer::new();
        req.encode_body(&mut w);
        let reply = handle_parcel_request(&parcel, owner, &w.buf, 10).unwrap();
        assert_eq!(reply.parcel_data.sequence_id, 7);
        assert!(reply.parcel_data.snap_selection);

        assert!(handle_parcel_request(&parcel, owner, b"garbage", 10).is_none());
    }

    #[test]
    fn roundtrip() {
        let owner = Uuid::nil();
        let parcel = RegionParcel::new(256, owner);
        let props = parcel.properties(7, true, 10, owner);

        let mut w = Writer::new();
        props.encode_body(&mut w);
        let decoded = msgs::ParcelProperties::decode(&w.buf).unwrap();
        assert_eq!(decoded, props);
    }

    #[test]
    fn flags_bit_zero() {
        assert!(PARCEL_FLAGS_DEFAULT & 1 == 1);
    }

    #[test]
    fn bitmap_512() {
        let parcel = RegionParcel::new(512, Uuid::nil());
        let props = parcel.properties(0, false, 0, Uuid::nil());
        assert_eq!(props.parcel_data.bitmap.len(), 2048);
    }
}
