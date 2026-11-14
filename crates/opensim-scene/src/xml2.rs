// SPDX-License-Identifier: GPL-3.0-or-later

use std::io::Cursor;
use std::str::FromStr;

use base64::Engine;
use glam::{Quat, Vec3};
use opensim_types::PermissionMask;
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use uuid::Uuid;

use crate::group::SceneObjectGroup;
use crate::part::{PartColor, PrimFlags, PrimShape, SceneObjectPart};

#[derive(Debug, thiserror::Error)]
pub enum XmlError {
    #[error("xml error: {0}")]
    Xml(#[from] quick_xml::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("missing element: {0}")]
    MissingElement(String),
    #[error("parse error: {0}")]
    Parse(String),
}

impl From<XmlError> for std::io::Error {
    fn from(e: XmlError) -> Self {
        match e {
            XmlError::Io(io) => io,
            other => std::io::Error::new(std::io::ErrorKind::Other, other.to_string()),
        }
    }
}

fn b64() -> base64::engine::GeneralPurpose {
    base64::engine::general_purpose::STANDARD
}

// ---------------------------------------------------------------------------
// Writer helpers
// ---------------------------------------------------------------------------

type W = Writer<Cursor<Vec<u8>>>;

fn write_simple(w: &mut W, tag: &str, val: &str) -> Result<(), XmlError> {
    w.create_element(tag)
        .write_text_content(BytesText::new(val))?;
    Ok(())
}

fn write_bool(w: &mut W, tag: &str, val: bool) -> Result<(), XmlError> {
    write_simple(w, tag, if val { "true" } else { "false" })
}

fn write_u8(w: &mut W, tag: &str, val: u8) -> Result<(), XmlError> {
    write_simple(w, tag, &val.to_string())
}

fn write_i8(w: &mut W, tag: &str, val: i8) -> Result<(), XmlError> {
    write_simple(w, tag, &val.to_string())
}

fn write_u16(w: &mut W, tag: &str, val: u16) -> Result<(), XmlError> {
    write_simple(w, tag, &val.to_string())
}

fn write_u32(w: &mut W, tag: &str, val: u32) -> Result<(), XmlError> {
    write_simple(w, tag, &val.to_string())
}

fn write_i32(w: &mut W, tag: &str, val: i32) -> Result<(), XmlError> {
    write_simple(w, tag, &val.to_string())
}

fn write_u64(w: &mut W, tag: &str, val: u64) -> Result<(), XmlError> {
    write_simple(w, tag, &val.to_string())
}

fn write_f32(w: &mut W, tag: &str, val: f32) -> Result<(), XmlError> {
    write_simple(w, tag, &val.to_string())
}

fn write_string(w: &mut W, tag: &str, val: &str) -> Result<(), XmlError> {
    w.create_element(tag)
        .write_text_content(BytesText::new(val))?;
    Ok(())
}

fn write_uuid(w: &mut W, tag: &str, uuid: &Uuid) -> Result<(), XmlError> {
    w.create_element(tag).write_inner_content(|w| {
        w.create_element("UUID")
            .write_text_content(BytesText::new(&uuid.to_string()))?;
        Ok(())
    })?;
    Ok(())
}

fn write_vec3(w: &mut W, tag: &str, v: &Vec3) -> Result<(), XmlError> {
    w.create_element(tag).write_inner_content(|w| {
        write_f32(w, "X", v.x)?;
        write_f32(w, "Y", v.y)?;
        write_f32(w, "Z", v.z)?;
        Ok(())
    })?;
    Ok(())
}

fn write_quat(w: &mut W, tag: &str, q: &Quat) -> Result<(), XmlError> {
    w.create_element(tag).write_inner_content(|w| {
        write_f32(w, "X", q.x)?;
        write_f32(w, "Y", q.y)?;
        write_f32(w, "Z", q.z)?;
        write_f32(w, "W", q.w)?;
        Ok(())
    })?;
    Ok(())
}

fn write_base64(w: &mut W, tag: &str, data: &[u8]) -> Result<(), XmlError> {
    let encoded = b64().encode(data);
    w.create_element(tag)
        .write_text_content(BytesText::new(&encoded))?;
    Ok(())
}

fn write_color(w: &mut W, tag: &str, c: &PartColor) -> Result<(), XmlError> {
    w.create_element(tag).write_inner_content(|w| {
        write_simple(w, "R", &c.r().to_string())?;
        write_simple(w, "G", &c.g().to_string())?;
        write_simple(w, "B", &c.b().to_string())?;
        write_simple(w, "A", &c.a().to_string())?;
        Ok(())
    })?;
    Ok(())
}

fn flags_to_string(flags: PrimFlags) -> String {
    let mut parts = Vec::new();
    if flags.contains(PrimFlags::PHYSICS) { parts.push("Physics"); }
    if flags.contains(PrimFlags::CREATE_SELECTED) { parts.push("CreateSelected"); }
    if flags.contains(PrimFlags::OBJECT_MODIFY) { parts.push("ObjectModify"); }
    if flags.contains(PrimFlags::OBJECT_COPY) { parts.push("ObjectCopy"); }
    if flags.contains(PrimFlags::OBJECT_ANY_OWNER) { parts.push("ObjectAnyOwner"); }
    if flags.contains(PrimFlags::OBJECT_YOU_OWNER) { parts.push("ObjectYouOwner"); }
    if flags.contains(PrimFlags::SCRIPTED) { parts.push("Scripted"); }
    if flags.contains(PrimFlags::TOUCH) { parts.push("Touch"); }
    if flags.contains(PrimFlags::OBJECT_MOVE) { parts.push("ObjectMove"); }
    if flags.contains(PrimFlags::TAKES_MONEY) { parts.push("Money"); }
    if flags.contains(PrimFlags::PHANTOM) { parts.push("Phantom"); }
    if flags.contains(PrimFlags::INVENTORY_EMPTY) { parts.push("InventoryEmpty"); }
    if flags.contains(PrimFlags::AFFECTS_NAVMESH) { parts.push("AffectsNavmesh"); }
    if flags.contains(PrimFlags::CHARACTER) { parts.push("Character"); }
    if flags.contains(PrimFlags::VOLUME_DETECT) { parts.push("VolumeDetect"); }
    if flags.contains(PrimFlags::INCLUDE_IN_SEARCH) { parts.push("IncludeInSearch"); }
    if flags.contains(PrimFlags::ALLOW_INVENTORY_DROP) { parts.push("AllowInventoryDrop"); }
    if flags.contains(PrimFlags::OBJECT_TRANSFER) { parts.push("ObjectTransfer"); }
    if flags.contains(PrimFlags::OBJECT_GROUP_OWNED) { parts.push("ObjectGroupOwned"); }
    if flags.contains(PrimFlags::OBJECT_YOU_OFFICER) { parts.push("ObjectYouOfficer"); }
    if flags.contains(PrimFlags::CAMERA_DECOUPLED) { parts.push("CameraDecoupled"); }
    if flags.contains(PrimFlags::ANIM_SOURCE) { parts.push("AnimSource"); }
    if flags.contains(PrimFlags::CAMERA_SOURCE) { parts.push("CameraSource"); }
    if flags.contains(PrimFlags::CAST_SHADOWS) { parts.push("CastShadows"); }
    if flags.contains(PrimFlags::OBJECT_OWNER_MODIFY) { parts.push("ObjectOwnerModify"); }
    if flags.contains(PrimFlags::TEMPORARY_ON_REZ) { parts.push("TemporaryOnRez"); }
    if flags.contains(PrimFlags::TEMPORARY) { parts.push("Temporary"); }
    if flags.contains(PrimFlags::ZONE_FLY) { parts.push("ZoneFly"); }
    if parts.is_empty() {
        String::from("None")
    } else {
        parts.join(" ")
    }
}

fn flags_from_string(s: &str) -> PrimFlags {
    let mut flags = PrimFlags::NONE;
    for word in s.split_whitespace() {
        let word = word.trim_matches(',');
        match word {
            "Physics" => flags |= PrimFlags::PHYSICS,
            "CreateSelected" => flags |= PrimFlags::CREATE_SELECTED,
            "ObjectModify" => flags |= PrimFlags::OBJECT_MODIFY,
            "ObjectCopy" => flags |= PrimFlags::OBJECT_COPY,
            "ObjectAnyOwner" => flags |= PrimFlags::OBJECT_ANY_OWNER,
            "ObjectYouOwner" => flags |= PrimFlags::OBJECT_YOU_OWNER,
            "Scripted" => flags |= PrimFlags::SCRIPTED,
            "Touch" => flags |= PrimFlags::TOUCH,
            "ObjectMove" => flags |= PrimFlags::OBJECT_MOVE,
            "Money" => flags |= PrimFlags::TAKES_MONEY,
            "Phantom" => flags |= PrimFlags::PHANTOM,
            "InventoryEmpty" => flags |= PrimFlags::INVENTORY_EMPTY,
            "AffectsNavmesh" => flags |= PrimFlags::AFFECTS_NAVMESH,
            "Character" => flags |= PrimFlags::CHARACTER,
            "VolumeDetect" => flags |= PrimFlags::VOLUME_DETECT,
            "IncludeInSearch" => flags |= PrimFlags::INCLUDE_IN_SEARCH,
            "AllowInventoryDrop" => flags |= PrimFlags::ALLOW_INVENTORY_DROP,
            "ObjectTransfer" => flags |= PrimFlags::OBJECT_TRANSFER,
            "ObjectGroupOwned" => flags |= PrimFlags::OBJECT_GROUP_OWNED,
            "ObjectYouOfficer" => flags |= PrimFlags::OBJECT_YOU_OFFICER,
            "CameraDecoupled" => flags |= PrimFlags::CAMERA_DECOUPLED,
            "AnimSource" => flags |= PrimFlags::ANIM_SOURCE,
            "CameraSource" => flags |= PrimFlags::CAMERA_SOURCE,
            "CastShadows" => flags |= PrimFlags::CAST_SHADOWS,
            "ObjectOwnerModify" => flags |= PrimFlags::OBJECT_OWNER_MODIFY,
            "TemporaryOnRez" => flags |= PrimFlags::TEMPORARY_ON_REZ,
            "Temporary" => flags |= PrimFlags::TEMPORARY,
            "ZoneFly" => flags |= PrimFlags::ZONE_FLY,
            "None" | "" => {}
            _ => {}
        }
    }
    flags
}

// ---------------------------------------------------------------------------
// Shape serialization
// ---------------------------------------------------------------------------

fn write_shape(w: &mut W, s: &PrimShape) -> Result<(), XmlError> {
    w.create_element("Shape").write_inner_content(|w| {
        write_u8(w, "ProfileCurve", s.profile_curve)?;
        write_base64(w, "TextureEntry", &s.texture_entry)?;
        write_base64(w, "ExtraParams", &s.extra_params)?;
        write_u16(w, "PathBegin", s.path_begin)?;
        write_u8(w, "PathCurve", s.path_curve)?;
        write_u16(w, "PathEnd", s.path_end)?;
        write_i8(w, "PathRadiusOffset", s.path_radius_offset)?;
        write_u8(w, "PathRevolutions", s.path_revolutions)?;
        write_u8(w, "PathScaleX", s.path_scale_x)?;
        write_u8(w, "PathScaleY", s.path_scale_y)?;
        write_u8(w, "PathShearX", s.path_shear_x)?;
        write_u8(w, "PathShearY", s.path_shear_y)?;
        write_i8(w, "PathSkew", s.path_skew)?;
        write_i8(w, "PathTaperX", s.path_taper_x)?;
        write_i8(w, "PathTaperY", s.path_taper_y)?;
        write_i8(w, "PathTwist", s.path_twist)?;
        write_i8(w, "PathTwistBegin", s.path_twist_begin)?;
        write_u8(w, "PCode", s.pcode)?;
        write_u16(w, "ProfileBegin", s.profile_begin)?;
        write_u16(w, "ProfileEnd", s.profile_end)?;
        write_u16(w, "ProfileHollow", s.profile_hollow)?;
        write_u8(w, "State", s.state)?;
        write_u8(w, "LastAttachPoint", s.last_attach_point)?;
        write_string(w, "ProfileShape", &s.profile_shape)?;
        write_string(w, "HollowShape", &s.hollow_shape)?;
        write_uuid(w, "SculptTexture", &s.sculpt_texture)?;
        write_u8(w, "SculptType", s.sculpt_type)?;
        write_i32(w, "FlexiSoftness", s.flexi_softness)?;
        write_f32(w, "FlexiTension", s.flexi_tension)?;
        write_f32(w, "FlexiDrag", s.flexi_drag)?;
        write_f32(w, "FlexiGravity", s.flexi_gravity)?;
        write_f32(w, "FlexiWind", s.flexi_wind)?;
        write_f32(w, "FlexiForceX", s.flexi_force_x)?;
        write_f32(w, "FlexiForceY", s.flexi_force_y)?;
        write_f32(w, "FlexiForceZ", s.flexi_force_z)?;
        write_f32(w, "LightColorR", s.light_color_r)?;
        write_f32(w, "LightColorG", s.light_color_g)?;
        write_f32(w, "LightColorB", s.light_color_b)?;
        write_f32(w, "LightColorA", s.light_color_a)?;
        write_f32(w, "LightRadius", s.light_radius)?;
        write_f32(w, "LightCutoff", s.light_cutoff)?;
        write_f32(w, "LightFalloff", s.light_falloff)?;
        write_f32(w, "LightIntensity", s.light_intensity)?;
        write_bool(w, "FlexiEntry", s.flexi_entry)?;
        write_bool(w, "LightEntry", s.light_entry)?;
        write_bool(w, "SculptEntry", s.sculpt_entry)?;
        if !s.media.is_empty() {
            write_string(w, "Media", &s.media)?;
        }
        if !s.mat_override.is_empty() {
            write_base64(w, "MatOvrd", &s.mat_override)?;
        }
        Ok(())
    })?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Part serialization
// ---------------------------------------------------------------------------

fn write_part(w: &mut W, p: &SceneObjectPart) -> Result<(), XmlError> {
    let mut elem = BytesStart::new("SceneObjectPart");
    elem.push_attribute(("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"));
    elem.push_attribute(("xmlns:xsd", "http://www.w3.org/2001/XMLSchema"));
    w.write_event(Event::Start(elem))?;

    write_bool(w, "AllowedDrop", p.allowed_drop)?;
    write_uuid(w, "CreatorID", &p.creator_id)?;
    if !p.creator_data.is_empty() {
        write_string(w, "CreatorData", &p.creator_data)?;
    }
    write_uuid(w, "FolderID", &p.folder_id)?;
    write_u32(w, "InventorySerial", p.inventory_serial)?;
    write_uuid(w, "UUID", &p.uuid)?;
    write_u32(w, "LocalId", p.local_id)?;
    write_string(w, "Name", &p.name)?;
    write_u8(w, "Material", p.material)?;
    write_bool(w, "PassTouches", p.pass_touches)?;
    write_bool(w, "PassCollisions", p.pass_collisions)?;
    write_u64(w, "RegionHandle", p.region_handle)?;
    write_i32(w, "ScriptAccessPin", p.script_access_pin)?;
    write_vec3(w, "GroupPosition", &p.group_position)?;
    write_vec3(w, "OffsetPosition", &p.offset_position)?;
    write_quat(w, "RotationOffset", &p.rotation_offset)?;
    write_vec3(w, "Velocity", &p.velocity)?;
    write_vec3(w, "AngularVelocity", &p.angular_velocity)?;
    write_vec3(w, "Acceleration", &p.acceleration)?;
    write_string(w, "Description", &p.description)?;
    write_color(w, "Color", &p.text_color)?;
    write_string(w, "Text", &p.text)?;
    write_string(w, "SitName", &p.sit_name)?;
    write_string(w, "TouchName", &p.touch_name)?;
    write_i32(w, "LinkNum", p.link_num)?;
    write_u8(w, "ClickAction", p.click_action)?;
    write_shape(w, &p.shape)?;
    write_vec3(w, "Scale", &p.scale)?;
    write_quat(w, "SitTargetOrientation", &p.sit_target_orientation)?;
    write_vec3(w, "SitTargetPosition", &p.sit_target_position)?;
    write_vec3(w, "SitTargetPositionLL", &p.sit_target_position_ll)?;
    write_quat(w, "SitTargetOrientationLL", &p.sit_target_orientation_ll)?;
    write_u32(w, "ParentID", p.parent_id)?;
    write_i32(w, "CreationDate", p.creation_date)?;
    write_u32(w, "Category", p.category)?;
    write_i32(w, "SalePrice", p.sale_price)?;
    write_u8(w, "ObjectSaleType", p.object_sale_type)?;
    write_i32(w, "OwnershipCost", p.ownership_cost)?;
    write_uuid(w, "GroupID", &p.group_id)?;
    write_uuid(w, "OwnerID", &p.owner_id)?;
    write_uuid(w, "LastOwnerID", &p.last_owner_id)?;
    write_uuid(w, "RezzerID", &p.rezzer_id)?;
    write_u32(w, "BaseMask", p.base_mask.bits())?;
    write_u32(w, "OwnerMask", p.owner_mask.bits())?;
    write_u32(w, "GroupMask", p.group_mask.bits())?;
    write_u32(w, "EveryoneMask", p.everyone_mask.bits())?;
    write_u32(w, "NextOwnerMask", p.next_owner_mask.bits())?;
    write_string(w, "Flags", &flags_to_string(p.flags))?;
    write_uuid(w, "CollisionSound", &p.collision_sound)?;
    write_f32(w, "CollisionSoundVolume", p.collision_sound_volume)?;
    if !p.media_url.is_empty() {
        write_string(w, "MediaUrl", &p.media_url)?;
    }
    write_vec3(w, "AttachedPos", &p.attached_pos)?;
    if !p.texture_animation.is_empty() {
        write_base64(w, "TextureAnimation", &p.texture_animation)?;
    }
    if !p.particle_system.is_empty() {
        write_base64(w, "ParticleSystem", &p.particle_system)?;
    }
    for (i, &price) in p.pay_price.iter().enumerate() {
        write_i32(w, &format!("PayPrice{i}"), price)?;
    }
    write_f32(w, "Buoyancy", p.buoyancy)?;
    write_vec3(w, "Force", &p.force)?;
    write_vec3(w, "Torque", &p.torque)?;
    write_bool(w, "VolumeDetectActive", p.volume_detect_active)?;
    write_u8(w, "PhysicsShapeType", p.physics_shape_type)?;
    write_f32(w, "Density", p.density)?;
    write_f32(w, "Friction", p.friction)?;
    write_f32(w, "Bounce", p.bounce)?;
    write_f32(w, "GravityModifier", p.gravity_modifier)?;
    write_vec3(w, "CameraEyeOffset", &p.camera_eye_offset)?;
    write_vec3(w, "CameraAtOffset", &p.camera_at_offset)?;
    write_uuid(w, "SoundID", &p.sound_id)?;
    write_f32(w, "SoundGain", p.sound_gain)?;
    write_u8(w, "SoundFlags", p.sound_flags)?;
    write_f32(w, "SoundRadius", p.sound_radius)?;
    write_bool(w, "SoundQueueing", p.sound_queueing)?;
    if p.sit_act_range != 0.0 {
        write_f32(w, "SitActRange", p.sit_act_range)?;
    }

    w.write_event(Event::End(BytesEnd::new("SceneObjectPart")))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Group serialization
// ---------------------------------------------------------------------------

pub fn serialize_group(group: &SceneObjectGroup) -> Result<String, XmlError> {
    let mut writer = Writer::new(Cursor::new(Vec::new()));

    writer.write_event(Event::Start(BytesStart::new("SceneObjectGroup")))?;

    write_part(&mut writer, group.root_part())?;

    let children: Vec<&SceneObjectPart> = group.parts().skip(1).collect();
    if !children.is_empty() {
        writer.write_event(Event::Start(BytesStart::new("OtherParts")))?;
        for child in children {
            write_part(&mut writer, child)?;
        }
        writer.write_event(Event::End(BytesEnd::new("OtherParts")))?;
    }

    writer.write_event(Event::End(BytesEnd::new("SceneObjectGroup")))?;

    let inner = writer.into_inner().into_inner();
    String::from_utf8(inner).map_err(|e| XmlError::Parse(e.to_string()))
}

// ---------------------------------------------------------------------------
// Reader helpers
// ---------------------------------------------------------------------------

fn read_text(reader: &mut Reader<&[u8]>) -> Result<String, XmlError> {
    let mut buf = Vec::new();
    let mut text = String::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Text(e) => {
                text.push_str(&e.unescape()?);
            }
            Event::End(_) => break,
            Event::Start(_) => {
                let inner = read_text(reader)?;
                if text.is_empty() {
                    text = inner;
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(text)
}

fn read_uuid_field(reader: &mut Reader<&[u8]>) -> Result<Uuid, XmlError> {
    let mut buf = Vec::new();
    let mut result = Uuid::nil();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag == "UUID" || tag == "Guid" {
                    let inner = read_text(reader)?;
                    result = Uuid::from_str(inner.trim())
                        .map_err(|e| XmlError::Parse(format!("uuid: {e}")))?;
                }
            }
            Event::Text(e) => {
                let s = e.unescape()?.trim().to_string();
                if !s.is_empty() {
                    if let Ok(u) = Uuid::from_str(&s) {
                        result = u;
                    }
                }
            }
            Event::End(_) => break,
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(result)
}

fn read_vec3_field(reader: &mut Reader<&[u8]>) -> Result<Vec3, XmlError> {
    let mut buf = Vec::new();
    let mut x = 0.0f32;
    let mut y = 0.0f32;
    let mut z = 0.0f32;
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let val = read_text(reader)?;
                let f: f32 = val.trim().parse().unwrap_or(0.0);
                match tag.as_str() {
                    "X" => x = f,
                    "Y" => y = f,
                    "Z" => z = f,
                    _ => {}
                }
            }
            Event::End(_) => break,
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(Vec3::new(x, y, z))
}

fn read_quat_field(reader: &mut Reader<&[u8]>) -> Result<Quat, XmlError> {
    let mut buf = Vec::new();
    let mut x = 0.0f32;
    let mut y = 0.0f32;
    let mut z = 0.0f32;
    let mut w = 1.0f32;
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let val = read_text(reader)?;
                let f: f32 = val.trim().parse().unwrap_or(0.0);
                match tag.as_str() {
                    "X" => x = f,
                    "Y" => y = f,
                    "Z" => z = f,
                    "W" => w = f,
                    _ => {}
                }
            }
            Event::End(_) => break,
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(Quat::from_xyzw(x, y, z, w))
}

fn read_color_field(reader: &mut Reader<&[u8]>) -> Result<PartColor, XmlError> {
    let mut buf = Vec::new();
    let mut r = 255u8;
    let mut g = 255u8;
    let mut b = 255u8;
    let mut a = 255u8;
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let val = read_text(reader)?;
                let v: u8 = val.trim().parse().unwrap_or(255);
                match tag.as_str() {
                    "R" => r = v,
                    "G" => g = v,
                    "B" => b = v,
                    "A" => a = v,
                    _ => {}
                }
            }
            Event::End(_) => break,
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(PartColor::new(r, g, b, a))
}

fn skip_element(reader: &mut Reader<&[u8]>) -> Result<(), XmlError> {
    let mut buf = Vec::new();
    let mut depth = 1u32;
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(_) => depth += 1,
            Event::End(_) => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Shape deserialization
// ---------------------------------------------------------------------------

fn read_shape(reader: &mut Reader<&[u8]>) -> Result<PrimShape, XmlError> {
    let mut s = PrimShape::default();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match tag.as_str() {
                    "ProfileCurve" => s.profile_curve = read_text(reader)?.trim().parse().unwrap_or(0),
                    "TextureEntry" => {
                        let txt = read_text(reader)?;
                        s.texture_entry = b64().decode(txt.trim()).unwrap_or_default();
                    }
                    "ExtraParams" => {
                        let txt = read_text(reader)?;
                        s.extra_params = b64().decode(txt.trim()).unwrap_or_default();
                    }
                    "PathBegin" => s.path_begin = read_text(reader)?.trim().parse().unwrap_or(0),
                    "PathCurve" => s.path_curve = read_text(reader)?.trim().parse().unwrap_or(0),
                    "PathEnd" => s.path_end = read_text(reader)?.trim().parse().unwrap_or(0),
                    "PathRadiusOffset" => s.path_radius_offset = read_text(reader)?.trim().parse().unwrap_or(0),
                    "PathRevolutions" => s.path_revolutions = read_text(reader)?.trim().parse().unwrap_or(0),
                    "PathScaleX" => s.path_scale_x = read_text(reader)?.trim().parse().unwrap_or(0),
                    "PathScaleY" => s.path_scale_y = read_text(reader)?.trim().parse().unwrap_or(0),
                    "PathShearX" => s.path_shear_x = read_text(reader)?.trim().parse().unwrap_or(0),
                    "PathShearY" => s.path_shear_y = read_text(reader)?.trim().parse().unwrap_or(0),
                    "PathSkew" => s.path_skew = read_text(reader)?.trim().parse().unwrap_or(0),
                    "PathTaperX" => s.path_taper_x = read_text(reader)?.trim().parse().unwrap_or(0),
                    "PathTaperY" => s.path_taper_y = read_text(reader)?.trim().parse().unwrap_or(0),
                    "PathTwist" => s.path_twist = read_text(reader)?.trim().parse().unwrap_or(0),
                    "PathTwistBegin" => s.path_twist_begin = read_text(reader)?.trim().parse().unwrap_or(0),
                    "PCode" => s.pcode = read_text(reader)?.trim().parse().unwrap_or(0),
                    "ProfileBegin" => s.profile_begin = read_text(reader)?.trim().parse().unwrap_or(0),
                    "ProfileEnd" => s.profile_end = read_text(reader)?.trim().parse().unwrap_or(0),
                    "ProfileHollow" => s.profile_hollow = read_text(reader)?.trim().parse().unwrap_or(0),
                    "State" => s.state = read_text(reader)?.trim().parse().unwrap_or(0),
                    "LastAttachPoint" => s.last_attach_point = read_text(reader)?.trim().parse().unwrap_or(0),
                    "ProfileShape" => s.profile_shape = read_text(reader)?.trim().to_string(),
                    "HollowShape" => s.hollow_shape = read_text(reader)?.trim().to_string(),
                    "SculptTexture" => s.sculpt_texture = read_uuid_field(reader)?,
                    "SculptType" => s.sculpt_type = read_text(reader)?.trim().parse().unwrap_or(0),
                    "FlexiSoftness" => s.flexi_softness = read_text(reader)?.trim().parse().unwrap_or(0),
                    "FlexiTension" => s.flexi_tension = read_text(reader)?.trim().parse().unwrap_or(0.0),
                    "FlexiDrag" => s.flexi_drag = read_text(reader)?.trim().parse().unwrap_or(0.0),
                    "FlexiGravity" => s.flexi_gravity = read_text(reader)?.trim().parse().unwrap_or(0.0),
                    "FlexiWind" => s.flexi_wind = read_text(reader)?.trim().parse().unwrap_or(0.0),
                    "FlexiForceX" => s.flexi_force_x = read_text(reader)?.trim().parse().unwrap_or(0.0),
                    "FlexiForceY" => s.flexi_force_y = read_text(reader)?.trim().parse().unwrap_or(0.0),
                    "FlexiForceZ" => s.flexi_force_z = read_text(reader)?.trim().parse().unwrap_or(0.0),
                    "LightColorR" => s.light_color_r = read_text(reader)?.trim().parse().unwrap_or(0.0),
                    "LightColorG" => s.light_color_g = read_text(reader)?.trim().parse().unwrap_or(0.0),
                    "LightColorB" => s.light_color_b = read_text(reader)?.trim().parse().unwrap_or(0.0),
                    "LightColorA" => s.light_color_a = read_text(reader)?.trim().parse().unwrap_or(1.0),
                    "LightRadius" => s.light_radius = read_text(reader)?.trim().parse().unwrap_or(0.0),
                    "LightCutoff" => s.light_cutoff = read_text(reader)?.trim().parse().unwrap_or(0.0),
                    "LightFalloff" => s.light_falloff = read_text(reader)?.trim().parse().unwrap_or(0.0),
                    "LightIntensity" => s.light_intensity = read_text(reader)?.trim().parse().unwrap_or(0.0),
                    "FlexiEntry" => s.flexi_entry = read_text(reader)?.trim() == "true",
                    "LightEntry" => s.light_entry = read_text(reader)?.trim() == "true",
                    "SculptEntry" => s.sculpt_entry = read_text(reader)?.trim() == "true",
                    "Media" => s.media = read_text(reader)?.trim().to_string(),
                    "MatOvrd" => {
                        let txt = read_text(reader)?;
                        s.mat_override = b64().decode(txt.trim()).unwrap_or_default();
                    }
                    _ => skip_element(reader)?,
                }
            }
            Event::End(_) => break,
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(s)
}

// ---------------------------------------------------------------------------
// Part deserialization
// ---------------------------------------------------------------------------

fn read_part(reader: &mut Reader<&[u8]>) -> Result<SceneObjectPart, XmlError> {
    let mut p = SceneObjectPart::default();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match tag.as_str() {
                    "AllowedDrop" => p.allowed_drop = read_text(reader)?.trim() == "true",
                    "CreatorID" => p.creator_id = read_uuid_field(reader)?,
                    "CreatorData" => p.creator_data = read_text(reader)?.trim().to_string(),
                    "FolderID" => p.folder_id = read_uuid_field(reader)?,
                    "InventorySerial" => p.inventory_serial = read_text(reader)?.trim().parse().unwrap_or(0),
                    "UUID" => p.uuid = read_uuid_field(reader)?,
                    "LocalId" => p.local_id = read_text(reader)?.trim().parse().unwrap_or(0),
                    "Name" => p.name = read_text(reader)?,
                    "Material" => p.material = read_text(reader)?.trim().parse().unwrap_or(0),
                    "PassTouches" => p.pass_touches = read_text(reader)?.trim() == "true",
                    "PassCollisions" => p.pass_collisions = read_text(reader)?.trim() == "true",
                    "RegionHandle" => p.region_handle = read_text(reader)?.trim().parse().unwrap_or(0),
                    "ScriptAccessPin" => p.script_access_pin = read_text(reader)?.trim().parse().unwrap_or(0),
                    "GroupPosition" => p.group_position = read_vec3_field(reader)?,
                    "OffsetPosition" => p.offset_position = read_vec3_field(reader)?,
                    "RotationOffset" => p.rotation_offset = read_quat_field(reader)?,
                    "Velocity" => p.velocity = read_vec3_field(reader)?,
                    "AngularVelocity" => p.angular_velocity = read_vec3_field(reader)?,
                    "Acceleration" => p.acceleration = read_vec3_field(reader)?,
                    "Description" => p.description = read_text(reader)?,
                    "Color" => p.text_color = read_color_field(reader)?,
                    "Text" => p.text = read_text(reader)?,
                    "SitName" => p.sit_name = read_text(reader)?,
                    "TouchName" => p.touch_name = read_text(reader)?,
                    "LinkNum" => p.link_num = read_text(reader)?.trim().parse().unwrap_or(0),
                    "ClickAction" => p.click_action = read_text(reader)?.trim().parse().unwrap_or(0),
                    "Shape" => p.shape = read_shape(reader)?,
                    "Scale" => p.scale = read_vec3_field(reader)?,
                    "SitTargetOrientation" => p.sit_target_orientation = read_quat_field(reader)?,
                    "SitTargetPosition" => p.sit_target_position = read_vec3_field(reader)?,
                    "SitTargetPositionLL" => p.sit_target_position_ll = read_vec3_field(reader)?,
                    "SitTargetOrientationLL" => p.sit_target_orientation_ll = read_quat_field(reader)?,
                    "ParentID" => p.parent_id = read_text(reader)?.trim().parse().unwrap_or(0),
                    "CreationDate" => p.creation_date = read_text(reader)?.trim().parse().unwrap_or(0),
                    "Category" => p.category = read_text(reader)?.trim().parse().unwrap_or(0),
                    "SalePrice" => p.sale_price = read_text(reader)?.trim().parse().unwrap_or(0),
                    "ObjectSaleType" => p.object_sale_type = read_text(reader)?.trim().parse().unwrap_or(0),
                    "OwnershipCost" => p.ownership_cost = read_text(reader)?.trim().parse().unwrap_or(0),
                    "GroupID" => p.group_id = read_uuid_field(reader)?,
                    "OwnerID" => p.owner_id = read_uuid_field(reader)?,
                    "LastOwnerID" => p.last_owner_id = read_uuid_field(reader)?,
                    "RezzerID" => p.rezzer_id = read_uuid_field(reader)?,
                    "BaseMask" => p.base_mask = PermissionMask::from_bits_truncate(
                        read_text(reader)?.trim().parse().unwrap_or(0),
                    ),
                    "OwnerMask" => p.owner_mask = PermissionMask::from_bits_truncate(
                        read_text(reader)?.trim().parse().unwrap_or(0),
                    ),
                    "GroupMask" => p.group_mask = PermissionMask::from_bits_truncate(
                        read_text(reader)?.trim().parse().unwrap_or(0),
                    ),
                    "EveryoneMask" => p.everyone_mask = PermissionMask::from_bits_truncate(
                        read_text(reader)?.trim().parse().unwrap_or(0),
                    ),
                    "NextOwnerMask" => p.next_owner_mask = PermissionMask::from_bits_truncate(
                        read_text(reader)?.trim().parse().unwrap_or(0),
                    ),
                    "Flags" => p.flags = flags_from_string(&read_text(reader)?),
                    "CollisionSound" => p.collision_sound = read_uuid_field(reader)?,
                    "CollisionSoundVolume" => p.collision_sound_volume = read_text(reader)?.trim().parse().unwrap_or(0.0),
                    "MediaUrl" => p.media_url = read_text(reader)?,
                    "AttachedPos" => p.attached_pos = read_vec3_field(reader)?,
                    "TextureAnimation" => {
                        let txt = read_text(reader)?;
                        p.texture_animation = b64().decode(txt.trim()).unwrap_or_default();
                    }
                    "ParticleSystem" => {
                        let txt = read_text(reader)?;
                        p.particle_system = b64().decode(txt.trim()).unwrap_or_default();
                    }
                    "PayPrice0" => p.pay_price[0] = read_text(reader)?.trim().parse().unwrap_or(-2),
                    "PayPrice1" => p.pay_price[1] = read_text(reader)?.trim().parse().unwrap_or(-2),
                    "PayPrice2" => p.pay_price[2] = read_text(reader)?.trim().parse().unwrap_or(-2),
                    "PayPrice3" => p.pay_price[3] = read_text(reader)?.trim().parse().unwrap_or(-2),
                    "PayPrice4" => p.pay_price[4] = read_text(reader)?.trim().parse().unwrap_or(-2),
                    "Buoyancy" => p.buoyancy = read_text(reader)?.trim().parse().unwrap_or(0.0),
                    "Force" => p.force = read_vec3_field(reader)?,
                    "Torque" => p.torque = read_vec3_field(reader)?,
                    "VolumeDetectActive" => p.volume_detect_active = read_text(reader)?.trim() == "true",
                    "PhysicsShapeType" => p.physics_shape_type = read_text(reader)?.trim().parse().unwrap_or(0),
                    "Density" => p.density = read_text(reader)?.trim().parse().unwrap_or(1000.0),
                    "Friction" => p.friction = read_text(reader)?.trim().parse().unwrap_or(0.6),
                    "Bounce" => p.bounce = read_text(reader)?.trim().parse().unwrap_or(0.5),
                    "GravityModifier" => p.gravity_modifier = read_text(reader)?.trim().parse().unwrap_or(1.0),
                    "CameraEyeOffset" => p.camera_eye_offset = read_vec3_field(reader)?,
                    "CameraAtOffset" => p.camera_at_offset = read_vec3_field(reader)?,
                    "SoundID" => p.sound_id = read_uuid_field(reader)?,
                    "SoundGain" => p.sound_gain = read_text(reader)?.trim().parse().unwrap_or(0.0),
                    "SoundFlags" => p.sound_flags = read_text(reader)?.trim().parse().unwrap_or(0),
                    "SoundRadius" => p.sound_radius = read_text(reader)?.trim().parse().unwrap_or(0.0),
                    "SoundQueueing" => p.sound_queueing = read_text(reader)?.trim() == "true",
                    "SitActRange" => p.sit_act_range = read_text(reader)?.trim().parse().unwrap_or(0.0),
                    "StandTarget" => { let _ = read_vec3_field(reader)?; }
                    "TaskInventory" | "DynAttrs" | "Vehicle" | "PhysicsInertia"
                    | "RotationAxisLocks" | "SOPAnims" => skip_element(reader)?,
                    _ => skip_element(reader)?,
                }
            }
            Event::End(_) => break,
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(p)
}

// ---------------------------------------------------------------------------
// Group deserialization
// ---------------------------------------------------------------------------

pub fn deserialize_group(xml: &str) -> Result<SceneObjectGroup, XmlError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut root_part: Option<SceneObjectPart> = None;
    let mut children: Vec<SceneObjectPart> = Vec::new();
    let mut in_other_parts = false;
    let mut found_root = false;

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match tag.as_str() {
                    "SceneObjectGroup" => {}
                    "SceneObjectPart" => {
                        let part = read_part(&mut reader)?;
                        if in_other_parts {
                            children.push(part);
                        } else if !found_root {
                            root_part = Some(part);
                            found_root = true;
                        } else {
                            children.push(part);
                        }
                    }
                    "OtherParts" => {
                        in_other_parts = true;
                    }
                    "RootPart" => {}
                    "Part" => {}
                    "KeyframeMotion" | "lnkstdt" => skip_element(&mut reader)?,
                    _ => skip_element(&mut reader)?,
                }
            }
            Event::End(e) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag == "OtherParts" {
                    in_other_parts = false;
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    let root = root_part.ok_or_else(|| XmlError::MissingElement("root SceneObjectPart".into()))?;
    let mut group = SceneObjectGroup::new(root);
    for child in children {
        group.add_part(child);
    }
    Ok(group)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::part::SceneObjectPart;

    fn make_test_part(name: &str, owner: Uuid) -> SceneObjectPart {
        let mut p = SceneObjectPart::new(name, owner);
        p.group_position = Vec3::new(128.0, 128.0, 25.0);
        p.scale = Vec3::new(1.0, 2.0, 3.0);
        p.rotation_offset = Quat::from_xyzw(0.0, 0.0, 0.707, 0.707);
        p.density = 500.0;
        p.friction = 0.3;
        p.bounce = 0.8;
        p.flags = PrimFlags::PHYSICS | PrimFlags::PHANTOM;
        p.text_color = PartColor::new(200, 100, 50, 255);
        p
    }

    #[test]
    fn roundtrip_single_prim() {
        let owner = Uuid::new_v4();
        let root = make_test_part("Round Trip Box", owner);
        let root_uuid = root.uuid;
        let group = SceneObjectGroup::new(root);

        let xml = serialize_group(&group).unwrap();
        let restored = deserialize_group(&xml).unwrap();

        assert_eq!(restored.uuid(), root_uuid);
        assert_eq!(restored.root_part().name, "Round Trip Box");
        assert_eq!(restored.root_part().owner_id, owner);
        assert_eq!(restored.root_part().group_position, Vec3::new(128.0, 128.0, 25.0));
        assert_eq!(restored.root_part().scale, Vec3::new(1.0, 2.0, 3.0));
        assert!((restored.root_part().rotation_offset.x - 0.0).abs() < 0.001);
        assert!((restored.root_part().rotation_offset.z - 0.707).abs() < 0.001);
        assert_eq!(restored.root_part().density, 500.0);
        assert_eq!(restored.root_part().friction, 0.3);
        assert_eq!(restored.root_part().bounce, 0.8);
        assert!(restored.root_part().flags.contains(PrimFlags::PHYSICS));
        assert!(restored.root_part().flags.contains(PrimFlags::PHANTOM));
        assert_eq!(restored.root_part().text_color, PartColor::new(200, 100, 50, 255));
        assert_eq!(restored.part_count(), 1);
    }

    #[test]
    fn roundtrip_linked_object() {
        let owner = Uuid::new_v4();
        let root = make_test_part("Root", owner);
        let root_uuid = root.uuid;
        let child1 = make_test_part("Child1", owner);
        let child1_uuid = child1.uuid;
        let child2 = make_test_part("Child2", owner);
        let child2_uuid = child2.uuid;

        let mut group = SceneObjectGroup::new(root);
        group.add_part(child1);
        group.add_part(child2);

        let xml = serialize_group(&group).unwrap();
        let restored = deserialize_group(&xml).unwrap();

        assert_eq!(restored.uuid(), root_uuid);
        assert_eq!(restored.part_count(), 3);
        assert_eq!(restored.root_part().name, "Root");
        assert!(restored.get_part(&child1_uuid).is_some());
        assert!(restored.get_part(&child2_uuid).is_some());
    }

    #[test]
    fn serialize_contains_expected_elements() {
        let owner = Uuid::new_v4();
        let root = make_test_part("Test Prim", owner);
        let group = SceneObjectGroup::new(root);
        let xml = serialize_group(&group).unwrap();

        assert!(xml.contains("<SceneObjectGroup>"));
        assert!(xml.contains("</SceneObjectGroup>"));
        assert!(xml.contains("<SceneObjectPart"));
        assert!(xml.contains("xmlns:xsi="));
        assert!(xml.contains("<Name>Test Prim</Name>"));
        assert!(xml.contains("<UUID>"));
        assert!(xml.contains("<GroupPosition>"));
        assert!(xml.contains("<Scale>"));
        assert!(xml.contains("<Shape>"));
        assert!(xml.contains("<Density>500</Density>"));
        assert!(xml.contains("<Flags>Physics Phantom</Flags>"));
    }

    #[test]
    fn deserialize_hand_written_xml() {
        let uuid_str = "12345678-1234-1234-1234-123456789abc";
        let owner_str = "aabbccdd-aabb-aabb-aabb-aabbccddaabb";
        let xml = format!(
            r#"<SceneObjectGroup>
  <SceneObjectPart xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xsd="http://www.w3.org/2001/XMLSchema">
    <AllowedDrop>false</AllowedDrop>
    <CreatorID><UUID>{owner_str}</UUID></CreatorID>
    <FolderID><UUID>00000000-0000-0000-0000-000000000000</UUID></FolderID>
    <InventorySerial>0</InventorySerial>
    <UUID><UUID>{uuid_str}</UUID></UUID>
    <LocalId>42</LocalId>
    <Name>Hand Written</Name>
    <Material>3</Material>
    <PassTouches>false</PassTouches>
    <PassCollisions>false</PassCollisions>
    <RegionHandle>1099511628032</RegionHandle>
    <ScriptAccessPin>0</ScriptAccessPin>
    <GroupPosition><X>100</X><Y>200</Y><Z>30</Z></GroupPosition>
    <OffsetPosition><X>0</X><Y>0</Y><Z>0</Z></OffsetPosition>
    <RotationOffset><X>0</X><Y>0</Y><Z>0</Z><W>1</W></RotationOffset>
    <Velocity><X>0</X><Y>0</Y><Z>0</Z></Velocity>
    <AngularVelocity><X>0</X><Y>0</Y><Z>0</Z></AngularVelocity>
    <Acceleration><X>0</X><Y>0</Y><Z>0</Z></Acceleration>
    <Description>test object</Description>
    <Color><R>128</R><G>64</G><B>32</B><A>255</A></Color>
    <Text></Text>
    <SitName></SitName>
    <TouchName></TouchName>
    <LinkNum>0</LinkNum>
    <ClickAction>0</ClickAction>
    <Shape>
      <ProfileCurve>5</ProfileCurve>
      <TextureEntry></TextureEntry>
      <ExtraParams></ExtraParams>
      <PathBegin>0</PathBegin>
      <PathCurve>32</PathCurve>
      <PathEnd>0</PathEnd>
      <PathRadiusOffset>0</PathRadiusOffset>
      <PathRevolutions>0</PathRevolutions>
      <PathScaleX>100</PathScaleX>
      <PathScaleY>100</PathScaleY>
      <PathShearX>0</PathShearX>
      <PathShearY>0</PathShearY>
      <PathSkew>0</PathSkew>
      <PathTaperX>0</PathTaperX>
      <PathTaperY>0</PathTaperY>
      <PathTwist>0</PathTwist>
      <PathTwistBegin>0</PathTwistBegin>
      <PCode>9</PCode>
      <ProfileBegin>0</ProfileBegin>
      <ProfileEnd>0</ProfileEnd>
      <ProfileHollow>0</ProfileHollow>
      <State>0</State>
      <LastAttachPoint>0</LastAttachPoint>
      <ProfileShape>Circle</ProfileShape>
      <HollowShape>Same</HollowShape>
      <SculptTexture><UUID>00000000-0000-0000-0000-000000000000</UUID></SculptTexture>
      <SculptType>0</SculptType>
      <FlexiSoftness>0</FlexiSoftness>
      <FlexiTension>0</FlexiTension>
      <FlexiDrag>0</FlexiDrag>
      <FlexiGravity>0</FlexiGravity>
      <FlexiWind>0</FlexiWind>
      <FlexiForceX>0</FlexiForceX>
      <FlexiForceY>0</FlexiForceY>
      <FlexiForceZ>0</FlexiForceZ>
      <LightColorR>0</LightColorR>
      <LightColorG>0</LightColorG>
      <LightColorB>0</LightColorB>
      <LightColorA>1</LightColorA>
      <LightRadius>0</LightRadius>
      <LightCutoff>0</LightCutoff>
      <LightFalloff>0</LightFalloff>
      <LightIntensity>0</LightIntensity>
      <FlexiEntry>false</FlexiEntry>
      <LightEntry>false</LightEntry>
      <SculptEntry>false</SculptEntry>
    </Shape>
    <Scale><X>0.5</X><Y>0.5</Y><Z>0.5</Z></Scale>
    <SitTargetOrientation><X>0</X><Y>0</Y><Z>0</Z><W>1</W></SitTargetOrientation>
    <SitTargetPosition><X>0</X><Y>0</Y><Z>0</Z></SitTargetPosition>
    <SitTargetPositionLL><X>0</X><Y>0</Y><Z>0</Z></SitTargetPositionLL>
    <SitTargetOrientationLL><X>0</X><Y>0</Y><Z>0</Z><W>1</W></SitTargetOrientationLL>
    <ParentID>0</ParentID>
    <CreationDate>1700000000</CreationDate>
    <Category>0</Category>
    <SalePrice>10</SalePrice>
    <ObjectSaleType>0</ObjectSaleType>
    <OwnershipCost>0</OwnershipCost>
    <GroupID><UUID>00000000-0000-0000-0000-000000000000</UUID></GroupID>
    <OwnerID><UUID>{owner_str}</UUID></OwnerID>
    <LastOwnerID><UUID>{owner_str}</UUID></LastOwnerID>
    <RezzerID><UUID>00000000-0000-0000-0000-000000000000</UUID></RezzerID>
    <BaseMask>581632</BaseMask>
    <OwnerMask>581632</OwnerMask>
    <GroupMask>0</GroupMask>
    <EveryoneMask>0</EveryoneMask>
    <NextOwnerMask>581632</NextOwnerMask>
    <Flags>None</Flags>
    <CollisionSound><UUID>00000000-0000-0000-0000-000000000000</UUID></CollisionSound>
    <CollisionSoundVolume>0</CollisionSoundVolume>
    <AttachedPos><X>0</X><Y>0</Y><Z>0</Z></AttachedPos>
    <PayPrice0>-2</PayPrice0>
    <PayPrice1>-2</PayPrice1>
    <PayPrice2>-2</PayPrice2>
    <PayPrice3>-2</PayPrice3>
    <PayPrice4>-2</PayPrice4>
    <Buoyancy>0</Buoyancy>
    <Force><X>0</X><Y>0</Y><Z>0</Z></Force>
    <Torque><X>0</X><Y>0</Y><Z>0</Z></Torque>
    <VolumeDetectActive>false</VolumeDetectActive>
    <PhysicsShapeType>0</PhysicsShapeType>
    <Density>1000</Density>
    <Friction>0.6</Friction>
    <Bounce>0.5</Bounce>
    <GravityModifier>1</GravityModifier>
    <CameraEyeOffset><X>0</X><Y>0</Y><Z>0</Z></CameraEyeOffset>
    <CameraAtOffset><X>0</X><Y>0</Y><Z>0</Z></CameraAtOffset>
    <SoundID><UUID>00000000-0000-0000-0000-000000000000</UUID></SoundID>
    <SoundGain>0</SoundGain>
    <SoundFlags>0</SoundFlags>
    <SoundRadius>0</SoundRadius>
    <SoundQueueing>false</SoundQueueing>
  </SceneObjectPart>
</SceneObjectGroup>"#
        );

        let group = deserialize_group(&xml).unwrap();
        let part = group.root_part();
        assert_eq!(part.uuid, Uuid::from_str(uuid_str).unwrap());
        assert_eq!(part.name, "Hand Written");
        assert_eq!(part.local_id, 42);
        assert_eq!(part.group_position, Vec3::new(100.0, 200.0, 30.0));
        assert_eq!(part.description, "test object");
        assert_eq!(part.text_color, PartColor::new(128, 64, 32, 255));
        assert_eq!(part.creation_date, 1700000000);
        assert_eq!(part.region_handle, 1099511628032);
        assert_eq!(part.owner_id, Uuid::from_str(owner_str).unwrap());
        assert_eq!(part.shape.profile_curve, 5);
        assert_eq!(part.shape.path_curve, 32);
        assert_eq!(part.shape.profile_shape, "Circle");
        assert_eq!(part.density, 1000.0);
        assert_eq!(part.flags, PrimFlags::NONE);
        assert_eq!(part.base_mask, PermissionMask::ALL);
    }

    #[test]
    fn flags_roundtrip() {
        let flags = PrimFlags::PHYSICS | PrimFlags::PHANTOM | PrimFlags::SCRIPTED;
        let s = flags_to_string(flags);
        assert_eq!(s, "Physics Scripted Phantom");
        let restored = flags_from_string(&s);
        assert_eq!(restored, flags);
    }

    #[test]
    fn flags_none_roundtrip() {
        let s = flags_to_string(PrimFlags::NONE);
        assert_eq!(s, "None");
        let restored = flags_from_string(&s);
        assert_eq!(restored, PrimFlags::NONE);
    }
}
