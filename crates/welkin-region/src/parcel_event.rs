// SPDX-License-Identifier: LGPL-3.0-or-later

use welkin_llsd::{Llsd, Map};

pub fn parcel_properties_event(p: &welkin_msg::msgs::ParcelProperties) -> Llsd {
    let mut top = Map::new();
    top.insert("ParcelData".into(), one_map(parcel_data(p)));
    top.insert("MediaData".into(), one_map(media_data()));
    top.insert("ParcelExtendedFlags".into(), one_map(parcel_extended_flags()));
    top.insert("AgeVerificationBlock".into(), one_map(age_verification_block(p)));
    top.insert("ParcelEnvironmentBlock".into(), one_map(parcel_environment_block(p)));
    top.insert("RegionAllowAccessBlock".into(), one_map(region_allow_access_block(p)));
    Llsd::Map(top)
}

fn one_map(m: Map) -> Llsd {
    Llsd::Array(vec![Llsd::Map(m)])
}

fn vec3(x: f32, y: f32, z: f32) -> Llsd {
    Llsd::Array(vec![
        Llsd::Real(x as f64),
        Llsd::Real(y as f64),
        Llsd::Real(z as f64),
    ])
}

fn bytes_to_string(bytes: &[u8]) -> String {
    let mut len = bytes.len();
    while len > 0 && bytes[len - 1] == 0 {
        len -= 1;
    }
    String::from_utf8_lossy(&bytes[..len]).into_owned()
}

fn parcel_data(p: &welkin_msg::msgs::ParcelProperties) -> Map {
    let d = &p.parcel_data;
    let mut m = Map::new();

    m.insert("LocalID".into(), Llsd::Integer(d.local_id));
    m.insert("AABBMax".into(), vec3(d.aabb_max.x, d.aabb_max.y, d.aabb_max.z));
    m.insert("AABBMin".into(), vec3(d.aabb_min.x, d.aabb_min.y, d.aabb_min.z));
    m.insert("Area".into(), Llsd::Integer(d.area));
    m.insert("AuctionID".into(), Llsd::Integer(d.auction_id as i32));
    m.insert("AuthBuyerID".into(), Llsd::Uuid(d.auth_buyer_id));
    m.insert("Bitmap".into(), Llsd::Binary(d.bitmap.clone()));
    m.insert("Category".into(), Llsd::Integer(d.category as i32));
    m.insert("ClaimDate".into(), Llsd::Date(d.claim_date as f64));
    m.insert("ClaimPrice".into(), Llsd::Integer(d.claim_price));
    m.insert("Desc".into(), Llsd::String(bytes_to_string(&d.desc)));
    m.insert("ParcelFlags".into(), Llsd::Integer(d.parcel_flags as i32));
    m.insert("GroupID".into(), Llsd::Uuid(d.group_id));
    m.insert("GroupPrims".into(), Llsd::Integer(d.group_prims));
    m.insert("IsGroupOwned".into(), Llsd::Boolean(d.is_group_owned));
    m.insert("LandingType".into(), Llsd::Integer(d.landing_type as i32));
    m.insert("MaxPrims".into(), Llsd::Integer(d.max_prims));
    m.insert("MediaID".into(), Llsd::Uuid(d.media_id));
    m.insert("MediaURL".into(), Llsd::String(bytes_to_string(&d.media_url)));
    m.insert("MediaAutoScale".into(), Llsd::Boolean(d.media_auto_scale != 0));
    m.insert("MusicURL".into(), Llsd::String(bytes_to_string(&d.music_url)));
    m.insert("Name".into(), Llsd::String(bytes_to_string(&d.name)));
    m.insert("OtherCleanTime".into(), Llsd::Integer(d.other_clean_time));
    m.insert("OtherCount".into(), Llsd::Integer(d.other_count));
    m.insert("OtherPrims".into(), Llsd::Integer(d.other_prims));
    m.insert("OwnerID".into(), Llsd::Uuid(d.owner_id));
    m.insert("OwnerPrims".into(), Llsd::Integer(d.owner_prims));
    m.insert("ParcelPrimBonus".into(), Llsd::Real(d.parcel_prim_bonus as f64));
    m.insert("PassHours".into(), Llsd::Real(d.pass_hours as f64));
    m.insert("PassPrice".into(), Llsd::Integer(d.pass_price));
    m.insert("PublicCount".into(), Llsd::Integer(d.public_count));
    m.insert("RegionDenyAnonymous".into(), Llsd::Boolean(d.region_deny_anonymous));
    m.insert("RegionDenyIdentified".into(), Llsd::Boolean(d.region_deny_identified));
    m.insert("RegionDenyTransacted".into(), Llsd::Boolean(d.region_deny_transacted));
    m.insert("RegionPushOverride".into(), Llsd::Boolean(d.region_push_override));
    m.insert("RentPrice".into(), Llsd::Integer(d.rent_price));
    m.insert("RequestResult".into(), Llsd::Integer(d.request_result));
    m.insert("SalePrice".into(), Llsd::Integer(d.sale_price));
    m.insert("SelectedPrims".into(), Llsd::Integer(d.selected_prims));
    m.insert("SelfCount".into(), Llsd::Integer(d.self_count));
    m.insert("SequenceID".into(), Llsd::Integer(d.sequence_id));
    m.insert("SimWideMaxPrims".into(), Llsd::Integer(d.sim_wide_max_prims));
    m.insert("SimWideTotalPrims".into(), Llsd::Integer(d.sim_wide_total_prims));
    m.insert("SnapSelection".into(), Llsd::Boolean(d.snap_selection));
    m.insert("SnapshotID".into(), Llsd::Uuid(d.snapshot_id));
    m.insert("Status".into(), Llsd::Integer(d.status as i32));
    m.insert("TotalPrims".into(), Llsd::Integer(d.total_prims));
    m.insert("UserLocation".into(), vec3(d.user_location.x, d.user_location.y, d.user_location.z));
    m.insert("UserLookAt".into(), vec3(d.user_look_at.x, d.user_look_at.y, d.user_look_at.z));
    m.insert("SeeAVs".into(), Llsd::Boolean(true));
    m.insert("AnyAVSounds".into(), Llsd::Boolean(true));
    m.insert("GroupAVSounds".into(), Llsd::Boolean(true));

    m
}

fn media_data() -> Map {
    let mut m = Map::new();
    m.insert("MediaDesc".into(), Llsd::String(String::new()));
    m.insert("MediaHeight".into(), Llsd::Integer(0));
    m.insert("MediaWidth".into(), Llsd::Integer(0));
    m.insert("MediaLoop".into(), Llsd::Boolean(false));
    m.insert("MediaType".into(), Llsd::String("none/none".into()));
    m.insert("ObscureMedia".into(), Llsd::Boolean(false));
    m.insert("ObscureMusic".into(), Llsd::Boolean(false));
    m
}

fn parcel_extended_flags() -> Map {
    let mut m = Map::new();
    m.insert("Flags".into(), Llsd::Integer(0));
    m
}

fn age_verification_block(p: &welkin_msg::msgs::ParcelProperties) -> Map {
    let mut m = Map::new();
    m.insert(
        "RegionDenyAgeUnverified".into(),
        Llsd::Boolean(p.age_verification_block.region_deny_age_unverified),
    );
    m
}

fn parcel_environment_block(p: &welkin_msg::msgs::ParcelProperties) -> Map {
    let b = &p.parcel_environment_block;
    let mut m = Map::new();
    m.insert("ParcelEnvironmentVersion".into(), Llsd::Integer(b.parcel_environment_version));
    m.insert(
        "RegionAllowEnvironmentOverride".into(),
        Llsd::Boolean(b.region_allow_environment_override),
    );
    m
}

fn region_allow_access_block(p: &welkin_msg::msgs::ParcelProperties) -> Map {
    let mut m = Map::new();
    m.insert(
        "RegionAllowAccessOverride".into(),
        Llsd::Boolean(p.region_allow_access_block.region_allow_access_override),
    );
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parcel_properties_event_shape() {
        let mut p = welkin_msg::msgs::ParcelProperties::default();
        p.parcel_data.local_id = 1;
        p.parcel_data.parcel_flags = 1;
        p.parcel_data.name = b"Test\0".to_vec();
        p.parcel_data.bitmap = vec![0xFFu8; 512];
        p.parcel_data.sequence_id = -10000;

        let llsd = parcel_properties_event(&p);

        let top = match &llsd {
            Llsd::Map(m) => m,
            _ => panic!("top-level must be a map"),
        };

        for key in [
            "ParcelData",
            "MediaData",
            "ParcelExtendedFlags",
            "AgeVerificationBlock",
            "ParcelEnvironmentBlock",
            "RegionAllowAccessBlock",
        ] {
            assert!(top.get(key).is_some(), "missing top-level key");
        }

        let parcel = match top.get("ParcelData") {
            Some(Llsd::Array(a)) if a.len() == 1 => match &a[0] {
                Llsd::Map(m) => m,
                _ => panic!("ParcelData entry must be a map"),
            },
            _ => panic!("ParcelData must be an array of one map"),
        };

        assert_eq!(parcel.get("ParcelFlags"), Some(&Llsd::Integer(1)));
        assert_eq!(parcel.get("Name"), Some(&Llsd::String("Test".into())));
        match parcel.get("Bitmap") {
            Some(Llsd::Binary(b)) => assert_eq!(b.len(), 512),
            _ => panic!("Bitmap must be binary of length 512"),
        }
        assert_eq!(parcel.get("SequenceID"), Some(&Llsd::Integer(-10000)));
        match parcel.get("AABBMax") {
            Some(Llsd::Array(a)) => {
                assert_eq!(a.len(), 3);
                assert!(matches!(a[0], Llsd::Real(_)));
                assert!(matches!(a[1], Llsd::Real(_)));
                assert!(matches!(a[2], Llsd::Real(_)));
            }
            _ => panic!("AABBMax must be an array of three reals"),
        }
    }
}
