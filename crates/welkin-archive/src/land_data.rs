// SPDX-License-Identifier: GPL-3.0-or-later

//! Land-data serialization for OAR archives.
//!
//! Ported from `OpenSim.Framework.Serialization.External.LandDataSerializer`.

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use quick_xml::escape::unescape;
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use std::io::Cursor;
use uuid::Uuid;

use crate::ArchiveError;

/// Land parcel data as stored in an OAR archive.
#[derive(Debug, Clone)]
pub struct ArchiveLandData {
    pub area: i32,
    pub auction_id: u32,
    pub auth_buyer_id: Uuid,
    pub category: i8,
    pub claim_date: i32,
    pub claim_price: i32,
    pub global_id: Uuid,
    pub group_id: Uuid,
    pub is_group_owned: bool,
    pub bitmap: Vec<u8>,
    pub description: String,
    pub flags: u32,
    pub landing_type: u8,
    pub name: String,
    pub status: i8,
    pub local_id: i32,
    pub media_auto_scale: u8,
    pub media_id: Uuid,
    pub media_url: String,
    pub music_url: String,
    pub owner_id: Uuid,
    pub access_list: Vec<LandAccessEntry>,
    pub pass_hours: f32,
    pub pass_price: i32,
    pub sale_price: i32,
    pub snapshot_id: Uuid,
    pub user_location: (f32, f32, f32),
    pub user_look_at: (f32, f32, f32),
    pub other_clean_time: i32,
    pub see_avs: bool,
    pub any_av_sounds: bool,
    pub group_av_sounds: bool,
}

impl Default for ArchiveLandData {
    fn default() -> Self {
        Self {
            area: 0,
            auction_id: 0,
            auth_buyer_id: Uuid::nil(),
            category: 0,
            claim_date: 0,
            claim_price: 0,
            global_id: Uuid::nil(),
            group_id: Uuid::nil(),
            is_group_owned: false,
            bitmap: Vec::new(),
            description: String::new(),
            flags: 0,
            landing_type: 0,
            name: String::new(),
            status: 0,
            local_id: 0,
            media_auto_scale: 0,
            media_id: Uuid::nil(),
            media_url: String::new(),
            music_url: String::new(),
            owner_id: Uuid::nil(),
            access_list: Vec::new(),
            pass_hours: 0.0,
            pass_price: 0,
            sale_price: 0,
            snapshot_id: Uuid::nil(),
            user_location: (0.0, 0.0, 0.0),
            user_look_at: (0.0, 0.0, 0.0),
            other_clean_time: 0,
            see_avs: true,
            any_av_sounds: true,
            group_av_sounds: true,
        }
    }
}

/// An entry in the parcel access list.
#[derive(Debug, Clone)]
pub struct LandAccessEntry {
    pub agent_id: Uuid,
    pub expires: u32,
    pub flags: u32,
}

// ---------------------------------------------------------------------------
// Serialization helpers
// ---------------------------------------------------------------------------

fn write_element(writer: &mut Writer<Cursor<Vec<u8>>>, name: &str, value: &str) -> Result<(), ArchiveError> {
    writer
        .create_element(name)
        .write_text_content(BytesText::new(value))
        .map_err(|e| ArchiveError::Xml(e.to_string()))?;
    Ok(())
}

/// Format a vector tuple matching the C# `Vector3.ToString()` output:
/// `<x, y, z>`
fn vec3_to_string(v: (f32, f32, f32)) -> String {
    format!("<{}, {}, {}>", v.0, v.1, v.2)
}

/// Parse a C# `Vector3.ToString()` format `<x, y, z>`.
fn parse_vec3(s: &str) -> Result<(f32, f32, f32), ArchiveError> {
    let s = s.trim();
    let s = s.strip_prefix('<').unwrap_or(s);
    let s = s.strip_suffix('>').unwrap_or(s);
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 3 {
        return Err(ArchiveError::Invalid(format!("bad Vector3: {s}")));
    }
    let x: f32 = parts[0]
        .trim()
        .parse()
        .map_err(|_| ArchiveError::Invalid(format!("bad Vector3 x: {}", parts[0])))?;
    let y: f32 = parts[1]
        .trim()
        .parse()
        .map_err(|_| ArchiveError::Invalid(format!("bad Vector3 y: {}", parts[1])))?;
    let z: f32 = parts[2]
        .trim()
        .parse()
        .map_err(|_| ArchiveError::Invalid(format!("bad Vector3 z: {}", parts[2])))?;
    Ok((x, y, z))
}

/// Serialize land data to the XML format used in OAR archives.
///
/// When `wipe_owners` is true, `OwnerID` and `GroupID` are written as
/// `Uuid::nil()` and `IsGroupOwned` as `false`, matching the C# "wipe-owners"
/// option.
pub fn serialize(data: &ArchiveLandData, wipe_owners: bool) -> Result<String, ArchiveError> {
    let mut writer = Writer::new(Cursor::new(Vec::new()));

    // XML declaration
    writer
        .write_event(Event::Decl(quick_xml::events::BytesDecl::new("1.0", Some("utf-8"), None)))
        .map_err(|e| ArchiveError::Xml(e.to_string()))?;

    // <LandData>
    writer
        .write_event(Event::Start(BytesStart::new("LandData")))
        .map_err(|e| ArchiveError::Xml(e.to_string()))?;

    write_element(&mut writer, "Area", &data.area.to_string())?;
    write_element(&mut writer, "AuctionID", &data.auction_id.to_string())?;
    write_element(&mut writer, "AuthBuyerID", &data.auth_buyer_id.to_string())?;
    write_element(&mut writer, "Category", &data.category.to_string())?;
    write_element(&mut writer, "ClaimDate", &data.claim_date.to_string())?;
    write_element(&mut writer, "ClaimPrice", &data.claim_price.to_string())?;
    write_element(&mut writer, "GlobalID", &data.global_id.to_string())?;

    let group_id = if wipe_owners { Uuid::nil() } else { data.group_id };
    write_element(&mut writer, "GroupID", &group_id.to_string())?;

    let is_group_owned = if wipe_owners { false } else { data.is_group_owned };
    write_element(&mut writer, "IsGroupOwned", &is_group_owned.to_string())?;

    write_element(&mut writer, "Bitmap", &BASE64.encode(&data.bitmap))?;
    write_element(&mut writer, "Description", &data.description)?;
    write_element(&mut writer, "Flags", &data.flags.to_string())?;
    write_element(&mut writer, "LandingType", &data.landing_type.to_string())?;
    write_element(&mut writer, "Name", &data.name)?;
    write_element(&mut writer, "Status", &data.status.to_string())?;
    write_element(&mut writer, "LocalID", &data.local_id.to_string())?;
    write_element(&mut writer, "MediaAutoScale", &data.media_auto_scale.to_string())?;
    write_element(&mut writer, "MediaID", &data.media_id.to_string())?;
    write_element(&mut writer, "MediaURL", &data.media_url)?;
    write_element(&mut writer, "MusicURL", &data.music_url)?;

    let owner_id = if wipe_owners { Uuid::nil() } else { data.owner_id };
    write_element(&mut writer, "OwnerID", &owner_id.to_string())?;

    // <ParcelAccessList>
    writer
        .write_event(Event::Start(BytesStart::new("ParcelAccessList")))
        .map_err(|e| ArchiveError::Xml(e.to_string()))?;
    for entry in &data.access_list {
        writer
            .write_event(Event::Start(BytesStart::new("ParcelAccessEntry")))
            .map_err(|e| ArchiveError::Xml(e.to_string()))?;
        write_element(&mut writer, "AgentID", &entry.agent_id.to_string())?;
        write_element(&mut writer, "Time", &entry.expires.to_string())?;
        write_element(&mut writer, "AccessList", &entry.flags.to_string())?;
        writer
            .write_event(Event::End(BytesEnd::new("ParcelAccessEntry")))
            .map_err(|e| ArchiveError::Xml(e.to_string()))?;
    }
    writer
        .write_event(Event::End(BytesEnd::new("ParcelAccessList")))
        .map_err(|e| ArchiveError::Xml(e.to_string()))?;

    write_element(&mut writer, "PassHours", &data.pass_hours.to_string())?;
    write_element(&mut writer, "PassPrice", &data.pass_price.to_string())?;
    write_element(&mut writer, "SalePrice", &data.sale_price.to_string())?;
    write_element(&mut writer, "SnapshotID", &data.snapshot_id.to_string())?;
    write_element(&mut writer, "UserLocation", &vec3_to_string(data.user_location))?;
    write_element(&mut writer, "UserLookAt", &vec3_to_string(data.user_look_at))?;
    write_element(&mut writer, "Dwell", "0")?;
    write_element(&mut writer, "OtherCleanTime", &data.other_clean_time.to_string())?;

    write_element(&mut writer, "SeeAVs", if data.see_avs { "1" } else { "0" })?;
    write_element(&mut writer, "AnyAVSnds", if data.any_av_sounds { "1" } else { "0" })?;
    write_element(&mut writer, "GrpAVSnds", if data.group_av_sounds { "1" } else { "0" })?;

    // </LandData>
    writer
        .write_event(Event::End(BytesEnd::new("LandData")))
        .map_err(|e| ArchiveError::Xml(e.to_string()))?;

    let buf = writer.into_inner().into_inner();
    String::from_utf8(buf).map_err(|e| ArchiveError::Xml(e.to_string()))
}

// ---------------------------------------------------------------------------
// Deserialization
// ---------------------------------------------------------------------------

/// Read element text and unescape XML entities.
fn read_text_unescaped(reader: &mut Reader<&[u8]>, name: quick_xml::name::QName<'_>) -> Result<String, ArchiveError> {
    let raw = reader
        .read_text(name)
        .map_err(|e| ArchiveError::Xml(e.to_string()))?;
    let unescaped = unescape(&raw)
        .map_err(|e| ArchiveError::Xml(e.to_string()))?;
    Ok(unescaped.into_owned())
}

/// Deserialize land data from OAR XML.
pub fn deserialize(xml: &str) -> Result<ArchiveLandData, ArchiveError> {
    let mut data = ArchiveLandData::default();
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut in_land_data = false;
    let mut in_access_list = false;
    let mut current_entry: Option<LandAccessEntry> = None;
    let mut in_access_entry = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = std::str::from_utf8(e.name().into_inner())
                    .map_err(|e| ArchiveError::Xml(e.to_string()))?;
                match name {
                    "LandData" => in_land_data = true,
                    "ParcelAccessList" => in_access_list = true,
                    "ParcelAccessEntry" => {
                        in_access_entry = true;
                        current_entry = Some(LandAccessEntry {
                            agent_id: Uuid::nil(),
                            expires: 0,
                            flags: 0,
                        });
                    }
                    _ if in_access_entry => {
                        let text = read_text_unescaped(&mut reader, e.name())?;
                        if let Some(ref mut entry) = current_entry {
                            apply_access_entry_field(entry, name, &text)?;
                        }
                    }
                    "Environment" if in_land_data => {
                        // Skip Environment element (not modelled)
                        let _text = read_text_unescaped(&mut reader, e.name())?;
                    }
                    _ if in_land_data && !in_access_list => {
                        let text = read_text_unescaped(&mut reader, e.name())?;
                        apply_land_field(&mut data, name, &text)?;
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let name = std::str::from_utf8(e.name().into_inner())
                    .map_err(|e| ArchiveError::Xml(e.to_string()))?;
                match name {
                    "LandData" => break,
                    "ParcelAccessList" => in_access_list = false,
                    "ParcelAccessEntry" => {
                        if let Some(entry) = current_entry.take() {
                            data.access_list.push(entry);
                        }
                        in_access_entry = false;
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(ArchiveError::Xml(e.to_string())),
            _ => {}
        }
        buf.clear();
    }

    Ok(data)
}

fn apply_land_field(
    data: &mut ArchiveLandData,
    name: &str,
    value: &str,
) -> Result<(), ArchiveError> {
    match name {
        "Area" => {
            data.area = value
                .parse()
                .map_err(|_| ArchiveError::Invalid(format!("bad Area: {value}")))?;
        }
        "AuctionID" => {
            data.auction_id = value
                .parse()
                .map_err(|_| ArchiveError::Invalid(format!("bad AuctionID: {value}")))?;
        }
        "AuthBuyerID" => data.auth_buyer_id = value.parse()?,
        "Category" => {
            data.category = value
                .parse()
                .map_err(|_| ArchiveError::Invalid(format!("bad Category: {value}")))?;
        }
        "ClaimDate" => {
            data.claim_date = value
                .parse()
                .map_err(|_| ArchiveError::Invalid(format!("bad ClaimDate: {value}")))?;
        }
        "ClaimPrice" => {
            data.claim_price = value
                .parse()
                .map_err(|_| ArchiveError::Invalid(format!("bad ClaimPrice: {value}")))?;
        }
        "GlobalID" => data.global_id = value.parse()?,
        "GroupID" => data.group_id = value.parse()?,
        "IsGroupOwned" => {
            data.is_group_owned = value.eq_ignore_ascii_case("true");
        }
        "Bitmap" => {
            data.bitmap = BASE64
                .decode(value)
                .map_err(|e| ArchiveError::Invalid(format!("bad Bitmap base64: {e}")))?;
        }
        "Description" => data.description = value.to_string(),
        "Flags" => {
            data.flags = value
                .parse()
                .map_err(|_| ArchiveError::Invalid(format!("bad Flags: {value}")))?;
        }
        "LandingType" => {
            data.landing_type = value
                .parse()
                .map_err(|_| ArchiveError::Invalid(format!("bad LandingType: {value}")))?;
        }
        "Name" => data.name = value.to_string(),
        "Status" => {
            data.status = value
                .parse()
                .map_err(|_| ArchiveError::Invalid(format!("bad Status: {value}")))?;
        }
        "LocalID" => {
            data.local_id = value
                .parse()
                .map_err(|_| ArchiveError::Invalid(format!("bad LocalID: {value}")))?;
        }
        "MediaAutoScale" => {
            data.media_auto_scale = value
                .parse()
                .map_err(|_| ArchiveError::Invalid(format!("bad MediaAutoScale: {value}")))?;
        }
        "MediaID" => data.media_id = value.parse()?,
        "MediaURL" => data.media_url = value.to_string(),
        "MusicURL" => data.music_url = value.to_string(),
        "OwnerID" => data.owner_id = value.parse()?,
        "PassHours" => {
            data.pass_hours = value
                .parse()
                .map_err(|_| ArchiveError::Invalid(format!("bad PassHours: {value}")))?;
        }
        "PassPrice" => {
            data.pass_price = value
                .parse()
                .map_err(|_| ArchiveError::Invalid(format!("bad PassPrice: {value}")))?;
        }
        "SalePrice" => {
            data.sale_price = value
                .parse()
                .map_err(|_| ArchiveError::Invalid(format!("bad SalePrice: {value}")))?;
        }
        "SnapshotID" => data.snapshot_id = value.parse()?,
        "UserLocation" => data.user_location = parse_vec3(value)?,
        "UserLookAt" => data.user_look_at = parse_vec3(value)?,
        "OtherCleanTime" => {
            data.other_clean_time = value
                .parse()
                .map_err(|_| ArchiveError::Invalid(format!("bad OtherCleanTime: {value}")))?;
        }
        "SeeAVs" => data.see_avs = value == "1",
        "AnyAVSnds" => data.any_av_sounds = value == "1",
        "GrpAVSnds" => data.group_av_sounds = value == "1",
        "Dwell" => {} // ignored on read, written as "0"
        _ => {}       // ignore unknown elements
    }
    Ok(())
}

fn apply_access_entry_field(
    entry: &mut LandAccessEntry,
    name: &str,
    value: &str,
) -> Result<(), ArchiveError> {
    match name {
        "AgentID" => entry.agent_id = value.parse()?,
        "Time" => {
            // Like C#, we ignore the actual time value and set expires to 0
            // (treat all bans as permanent for OAR compat).
            let _ = value;
            entry.expires = 0;
        }
        "AccessList" => {
            entry.flags = value
                .parse()
                .map_err(|_| ArchiveError::Invalid(format!("bad AccessList flags: {value}")))?;
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_land_data() -> ArchiveLandData {
        ArchiveLandData {
            area: 512,
            auction_id: 0,
            auth_buyer_id: Uuid::nil(),
            category: 3,
            claim_date: 1700000000,
            claim_price: 0,
            global_id: Uuid::parse_str("11111111-2222-3333-4444-555555555555").unwrap(),
            group_id: Uuid::parse_str("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee").unwrap(),
            is_group_owned: true,
            bitmap: vec![0xFF, 0x00, 0xAB],
            description: "Test parcel".to_string(),
            flags: 100,
            landing_type: 2,
            name: "My Land".to_string(),
            status: 0,
            local_id: 42,
            media_auto_scale: 1,
            media_id: Uuid::nil(),
            media_url: "http://example.com/media".to_string(),
            music_url: "http://example.com/music".to_string(),
            owner_id: Uuid::parse_str("deadbeef-dead-beef-dead-beefdeadbeef").unwrap(),
            access_list: vec![
                LandAccessEntry {
                    agent_id: Uuid::parse_str("cafebabe-cafe-babe-cafe-babecafebabe").unwrap(),
                    expires: 0,
                    flags: 1,
                },
                LandAccessEntry {
                    agent_id: Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap(),
                    expires: 0,
                    flags: 2,
                },
            ],
            pass_hours: 1.5,
            pass_price: 10,
            sale_price: 1000,
            snapshot_id: Uuid::nil(),
            user_location: (128.0, 128.0, 25.0),
            user_look_at: (0.0, 1.0, 0.0),
            other_clean_time: 5,
            see_avs: false,
            any_av_sounds: false,
            group_av_sounds: true,
        }
    }

    #[test]
    fn serialize_deserialize_roundtrip() {
        let data = sample_land_data();
        let xml = serialize(&data, false).expect("serialize failed");
        let restored = deserialize(&xml).expect("deserialize failed");

        assert_eq!(restored.area, 512);
        assert_eq!(restored.auction_id, 0);
        assert_eq!(restored.category, 3);
        assert_eq!(restored.claim_date, 1700000000);
        assert_eq!(restored.global_id, data.global_id);
        assert_eq!(restored.group_id, data.group_id);
        assert_eq!(restored.is_group_owned, true);
        assert_eq!(restored.bitmap, vec![0xFF, 0x00, 0xAB]);
        assert_eq!(restored.description, "Test parcel");
        assert_eq!(restored.flags, 100);
        assert_eq!(restored.landing_type, 2);
        assert_eq!(restored.name, "My Land");
        assert_eq!(restored.status, 0);
        assert_eq!(restored.local_id, 42);
        assert_eq!(restored.media_auto_scale, 1);
        assert_eq!(restored.media_url, "http://example.com/media");
        assert_eq!(restored.music_url, "http://example.com/music");
        assert_eq!(restored.owner_id, data.owner_id);

        // Access list
        assert_eq!(restored.access_list.len(), 2);
        assert_eq!(restored.access_list[0].agent_id, data.access_list[0].agent_id);
        assert_eq!(restored.access_list[0].flags, 1);
        assert_eq!(restored.access_list[1].agent_id, data.access_list[1].agent_id);
        assert_eq!(restored.access_list[1].flags, 2);

        assert!((restored.pass_hours - 1.5).abs() < f32::EPSILON);
        assert_eq!(restored.pass_price, 10);
        assert_eq!(restored.sale_price, 1000);
        assert_eq!(restored.user_location.0, 128.0);
        assert_eq!(restored.user_location.1, 128.0);
        assert_eq!(restored.user_location.2, 25.0);
        assert_eq!(restored.other_clean_time, 5);
        assert_eq!(restored.see_avs, false);
        assert_eq!(restored.any_av_sounds, false);
        assert_eq!(restored.group_av_sounds, true);
    }

    #[test]
    fn wipe_owners_zeros_ids() {
        let data = sample_land_data();
        let xml = serialize(&data, true).expect("serialize failed");
        let restored = deserialize(&xml).expect("deserialize failed");

        assert_eq!(restored.owner_id, Uuid::nil());
        assert_eq!(restored.group_id, Uuid::nil());
        assert_eq!(restored.is_group_owned, false);
        // Other fields should be intact
        assert_eq!(restored.name, "My Land");
        assert_eq!(restored.global_id, data.global_id);
    }

    #[test]
    fn empty_access_list_roundtrip() {
        let mut data = sample_land_data();
        data.access_list.clear();
        let xml = serialize(&data, false).expect("serialize failed");
        let restored = deserialize(&xml).expect("deserialize failed");
        assert!(restored.access_list.is_empty());
    }

    #[test]
    fn xml_has_land_data_root() {
        let xml = serialize(&ArchiveLandData::default(), false).unwrap();
        assert!(xml.contains("<LandData>"));
        assert!(xml.contains("</LandData>"));
    }
}
