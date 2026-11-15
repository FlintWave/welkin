// SPDX-License-Identifier: LGPL-3.0-or-later

//! Region settings serialization for OAR archives.
//!
//! Ported from `OpenSim.Framework.Serialization.External.RegionSettingsSerializer`.

use quick_xml::escape::unescape;
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use std::io::Cursor;
use uuid::Uuid;

use crate::ArchiveError;

/// Region settings as stored in an OAR archive.
///
/// This is the archive-specific representation, not the database row struct.
#[derive(Debug, Clone)]
pub struct RegionSettings {
    // General
    pub allow_damage: bool,
    pub allow_land_resell: bool,
    pub allow_land_join_divide: bool,
    pub block_fly: bool,
    pub block_show_in_search: bool,
    pub block_terraform: bool,
    pub disable_collisions: bool,
    pub disable_physics: bool,
    pub disable_scripts: bool,
    pub maturity: i32,
    pub restrict_pushing: bool,
    pub agent_limit: i32,
    pub object_bonus: f64,
    // GroundTextures
    pub terrain_texture1: Uuid,
    pub terrain_texture2: Uuid,
    pub terrain_texture3: Uuid,
    pub terrain_texture4: Uuid,
    pub terrain_pbr1: Uuid,
    pub terrain_pbr2: Uuid,
    pub terrain_pbr3: Uuid,
    pub terrain_pbr4: Uuid,
    pub elevation1_sw: f64,
    pub elevation1_nw: f64,
    pub elevation1_se: f64,
    pub elevation1_ne: f64,
    pub elevation2_sw: f64,
    pub elevation2_nw: f64,
    pub elevation2_se: f64,
    pub elevation2_ne: f64,
    // Terrain
    pub water_height: f64,
    pub terrain_raise_limit: f64,
    pub terrain_lower_limit: f64,
    pub use_estate_sun: bool,
    pub fixed_sun: bool,
    pub sun_position: f64,
    // Telehub
    pub telehub_object: Uuid,
    pub spawn_points: Vec<String>,
    // Estate (merged)
    pub allow_direct_teleport: bool,
    pub allow_environment_override: bool,
}

impl Default for RegionSettings {
    fn default() -> Self {
        Self {
            allow_damage: false,
            allow_land_resell: true,
            allow_land_join_divide: true,
            block_fly: false,
            block_show_in_search: false,
            block_terraform: false,
            disable_collisions: false,
            disable_physics: false,
            disable_scripts: false,
            maturity: 0,
            restrict_pushing: false,
            agent_limit: 40,
            object_bonus: 1.0,
            terrain_texture1: Uuid::nil(),
            terrain_texture2: Uuid::nil(),
            terrain_texture3: Uuid::nil(),
            terrain_texture4: Uuid::nil(),
            terrain_pbr1: Uuid::nil(),
            terrain_pbr2: Uuid::nil(),
            terrain_pbr3: Uuid::nil(),
            terrain_pbr4: Uuid::nil(),
            elevation1_sw: 10.0,
            elevation1_nw: 10.0,
            elevation1_se: 10.0,
            elevation1_ne: 10.0,
            elevation2_sw: 60.0,
            elevation2_nw: 60.0,
            elevation2_se: 60.0,
            elevation2_ne: 60.0,
            water_height: 20.0,
            terrain_raise_limit: 100.0,
            terrain_lower_limit: -100.0,
            use_estate_sun: true,
            fixed_sun: false,
            sun_position: 0.0,
            telehub_object: Uuid::nil(),
            spawn_points: Vec::new(),
            allow_direct_teleport: true,
            allow_environment_override: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Serialization helpers
// ---------------------------------------------------------------------------

fn write_element(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    name: &str,
    value: &str,
) -> Result<(), ArchiveError> {
    writer
        .create_element(name)
        .write_text_content(BytesText::new(value))
        .map_err(|e| ArchiveError::Xml(e.to_string()))?;
    Ok(())
}

fn bool_str(v: bool) -> &'static str {
    if v { "True" } else { "False" }
}

/// Serialize region settings to the XML format used in OAR archives.
///
/// Produces indented XML with `<?xml version="1.0"?>` header, matching the
/// C# `RegionSettingsSerializer.Serialize` output.
pub fn serialize(settings: &RegionSettings) -> Result<String, ArchiveError> {
    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

    // XML declaration
    writer
        .write_event(Event::Decl(quick_xml::events::BytesDecl::new(
            "1.0", None, None,
        )))
        .map_err(|e| ArchiveError::Xml(e.to_string()))?;

    // <RegionSettings>
    writer
        .write_event(Event::Start(BytesStart::new("RegionSettings")))
        .map_err(|e| ArchiveError::Xml(e.to_string()))?;

    // <General>
    writer
        .write_event(Event::Start(BytesStart::new("General")))
        .map_err(|e| ArchiveError::Xml(e.to_string()))?;
    write_element(&mut writer, "AllowDamage", bool_str(settings.allow_damage))?;
    write_element(
        &mut writer,
        "AllowLandResell",
        bool_str(settings.allow_land_resell),
    )?;
    write_element(
        &mut writer,
        "AllowLandJoinDivide",
        bool_str(settings.allow_land_join_divide),
    )?;
    write_element(&mut writer, "BlockFly", bool_str(settings.block_fly))?;
    write_element(
        &mut writer,
        "BlockLandShowInSearch",
        bool_str(settings.block_show_in_search),
    )?;
    write_element(
        &mut writer,
        "BlockTerraform",
        bool_str(settings.block_terraform),
    )?;
    write_element(
        &mut writer,
        "DisableCollisions",
        bool_str(settings.disable_collisions),
    )?;
    write_element(
        &mut writer,
        "DisablePhysics",
        bool_str(settings.disable_physics),
    )?;
    write_element(
        &mut writer,
        "DisableScripts",
        bool_str(settings.disable_scripts),
    )?;
    write_element(
        &mut writer,
        "MaturityRating",
        &settings.maturity.to_string(),
    )?;
    write_element(
        &mut writer,
        "RestrictPushing",
        bool_str(settings.restrict_pushing),
    )?;
    write_element(&mut writer, "AgentLimit", &settings.agent_limit.to_string())?;
    write_element(
        &mut writer,
        "ObjectBonus",
        &settings.object_bonus.to_string(),
    )?;
    writer
        .write_event(Event::End(BytesEnd::new("General")))
        .map_err(|e| ArchiveError::Xml(e.to_string()))?;

    // <GroundTextures>
    writer
        .write_event(Event::Start(BytesStart::new("GroundTextures")))
        .map_err(|e| ArchiveError::Xml(e.to_string()))?;
    write_element(
        &mut writer,
        "Texture1",
        &settings.terrain_texture1.to_string(),
    )?;
    write_element(
        &mut writer,
        "Texture2",
        &settings.terrain_texture2.to_string(),
    )?;
    write_element(
        &mut writer,
        "Texture3",
        &settings.terrain_texture3.to_string(),
    )?;
    write_element(
        &mut writer,
        "Texture4",
        &settings.terrain_texture4.to_string(),
    )?;
    write_element(&mut writer, "PBR1", &settings.terrain_pbr1.to_string())?;
    write_element(&mut writer, "PBR2", &settings.terrain_pbr2.to_string())?;
    write_element(&mut writer, "PBR3", &settings.terrain_pbr3.to_string())?;
    write_element(&mut writer, "PBR4", &settings.terrain_pbr4.to_string())?;
    write_element(
        &mut writer,
        "ElevationLowSW",
        &settings.elevation1_sw.to_string(),
    )?;
    write_element(
        &mut writer,
        "ElevationLowNW",
        &settings.elevation1_nw.to_string(),
    )?;
    write_element(
        &mut writer,
        "ElevationLowSE",
        &settings.elevation1_se.to_string(),
    )?;
    write_element(
        &mut writer,
        "ElevationLowNE",
        &settings.elevation1_ne.to_string(),
    )?;
    write_element(
        &mut writer,
        "ElevationHighSW",
        &settings.elevation2_sw.to_string(),
    )?;
    write_element(
        &mut writer,
        "ElevationHighNW",
        &settings.elevation2_nw.to_string(),
    )?;
    write_element(
        &mut writer,
        "ElevationHighSE",
        &settings.elevation2_se.to_string(),
    )?;
    write_element(
        &mut writer,
        "ElevationHighNE",
        &settings.elevation2_ne.to_string(),
    )?;
    writer
        .write_event(Event::End(BytesEnd::new("GroundTextures")))
        .map_err(|e| ArchiveError::Xml(e.to_string()))?;

    // <Terrain>
    writer
        .write_event(Event::Start(BytesStart::new("Terrain")))
        .map_err(|e| ArchiveError::Xml(e.to_string()))?;
    write_element(
        &mut writer,
        "WaterHeight",
        &settings.water_height.to_string(),
    )?;
    write_element(
        &mut writer,
        "TerrainRaiseLimit",
        &settings.terrain_raise_limit.to_string(),
    )?;
    write_element(
        &mut writer,
        "TerrainLowerLimit",
        &settings.terrain_lower_limit.to_string(),
    )?;
    write_element(
        &mut writer,
        "UseEstateSun",
        bool_str(settings.use_estate_sun),
    )?;
    write_element(&mut writer, "FixedSun", bool_str(settings.fixed_sun))?;
    write_element(
        &mut writer,
        "SunPosition",
        &settings.sun_position.to_string(),
    )?;
    writer
        .write_event(Event::End(BytesEnd::new("Terrain")))
        .map_err(|e| ArchiveError::Xml(e.to_string()))?;

    // <Telehub>
    if settings.telehub_object.is_nil() && settings.spawn_points.is_empty() {
        // Write empty element like C# does when TelehubObject is zero
        writer
            .write_event(Event::Start(BytesStart::new("Telehub")))
            .map_err(|e| ArchiveError::Xml(e.to_string()))?;
        writer
            .write_event(Event::End(BytesEnd::new("Telehub")))
            .map_err(|e| ArchiveError::Xml(e.to_string()))?;
    } else {
        writer
            .write_event(Event::Start(BytesStart::new("Telehub")))
            .map_err(|e| ArchiveError::Xml(e.to_string()))?;
        write_element(
            &mut writer,
            "TelehubObject",
            &settings.telehub_object.to_string(),
        )?;
        for sp in &settings.spawn_points {
            write_element(&mut writer, "SpawnPoint", sp)?;
        }
        writer
            .write_event(Event::End(BytesEnd::new("Telehub")))
            .map_err(|e| ArchiveError::Xml(e.to_string()))?;
    }

    // <Estate>
    writer
        .write_event(Event::Start(BytesStart::new("Estate")))
        .map_err(|e| ArchiveError::Xml(e.to_string()))?;
    write_element(
        &mut writer,
        "AllowDirectTeleport",
        bool_str(settings.allow_direct_teleport),
    )?;
    write_element(
        &mut writer,
        "AllowEnvironmentOverride",
        bool_str(settings.allow_environment_override),
    )?;
    writer
        .write_event(Event::End(BytesEnd::new("Estate")))
        .map_err(|e| ArchiveError::Xml(e.to_string()))?;

    // </RegionSettings>
    writer
        .write_event(Event::End(BytesEnd::new("RegionSettings")))
        .map_err(|e| ArchiveError::Xml(e.to_string()))?;

    let buf = writer.into_inner().into_inner();
    String::from_utf8(buf).map_err(|e| ArchiveError::Xml(e.to_string()))
}

// ---------------------------------------------------------------------------
// Deserialization
// ---------------------------------------------------------------------------

/// Parse a boolean from the C# `ToString()` format (`"True"` / `"False"`).
fn parse_bool(s: &str) -> bool {
    s.eq_ignore_ascii_case("true")
}

/// Current section during deserialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Root,
    General,
    GroundTextures,
    Terrain,
    Telehub,
    Estate,
}

/// Deserialize region settings from OAR XML.
pub fn deserialize(xml: &str) -> Result<RegionSettings, ArchiveError> {
    let mut settings = RegionSettings::default();
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut section = Section::Root;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = std::str::from_utf8(e.name().into_inner())
                    .map_err(|e| ArchiveError::Xml(e.to_string()))?;
                match name {
                    "RegionSettings" => {}
                    "General" => section = Section::General,
                    "GroundTextures" => section = Section::GroundTextures,
                    "Terrain" => section = Section::Terrain,
                    "Telehub" => section = Section::Telehub,
                    "Environment" => {
                        // Skip the Environment section (not modelled here)
                        skip_element(&mut reader, &mut buf)?;
                    }
                    "Estate" => section = Section::Estate,
                    _ => {
                        // Read the text content of leaf elements
                        let raw = reader
                            .read_text(e.name())
                            .map_err(|e| ArchiveError::Xml(e.to_string()))?;
                        let text = unescape(&raw).map_err(|e| ArchiveError::Xml(e.to_string()))?;
                        apply_field(&mut settings, section, name, &text)?;
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let name = std::str::from_utf8(e.name().into_inner())
                    .map_err(|e| ArchiveError::Xml(e.to_string()))?;
                match name {
                    "General" | "GroundTextures" | "Terrain" | "Telehub" | "Estate" => {
                        section = Section::Root;
                    }
                    "RegionSettings" => break,
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(ArchiveError::Xml(e.to_string())),
            _ => {}
        }
        buf.clear();
    }

    Ok(settings)
}

/// Skip over the current element and all its children.
fn skip_element(reader: &mut Reader<&[u8]>, buf: &mut Vec<u8>) -> Result<(), ArchiveError> {
    let mut depth: u32 = 1;
    loop {
        match reader.read_event_into(buf) {
            Ok(Event::Start(_)) => depth += 1,
            Ok(Event::End(_)) => {
                depth -= 1;
                if depth == 0 {
                    return Ok(());
                }
            }
            Ok(Event::Eof) => return Err(ArchiveError::Xml("unexpected EOF".into())),
            Err(e) => return Err(ArchiveError::Xml(e.to_string())),
            _ => {}
        }
        buf.clear();
    }
}

fn apply_field(
    settings: &mut RegionSettings,
    section: Section,
    name: &str,
    value: &str,
) -> Result<(), ArchiveError> {
    match section {
        Section::General => match name {
            "AllowDamage" => settings.allow_damage = parse_bool(value),
            "AllowLandResell" => settings.allow_land_resell = parse_bool(value),
            "AllowLandJoinDivide" => settings.allow_land_join_divide = parse_bool(value),
            "BlockFly" => settings.block_fly = parse_bool(value),
            "BlockLandShowInSearch" => settings.block_show_in_search = parse_bool(value),
            "BlockTerraform" => settings.block_terraform = parse_bool(value),
            "DisableCollisions" => settings.disable_collisions = parse_bool(value),
            "DisablePhysics" => settings.disable_physics = parse_bool(value),
            "DisableScripts" => settings.disable_scripts = parse_bool(value),
            "MaturityRating" => {
                settings.maturity = value
                    .parse()
                    .map_err(|_| ArchiveError::Invalid(format!("bad MaturityRating: {value}")))?;
            }
            "RestrictPushing" => settings.restrict_pushing = parse_bool(value),
            "AgentLimit" => {
                settings.agent_limit = value
                    .parse()
                    .map_err(|_| ArchiveError::Invalid(format!("bad AgentLimit: {value}")))?;
            }
            "ObjectBonus" => {
                settings.object_bonus = value
                    .parse()
                    .map_err(|_| ArchiveError::Invalid(format!("bad ObjectBonus: {value}")))?;
            }
            _ => {} // ignore unknown
        },
        Section::GroundTextures => match name {
            "Texture1" => settings.terrain_texture1 = value.parse()?,
            "Texture2" => settings.terrain_texture2 = value.parse()?,
            "Texture3" => settings.terrain_texture3 = value.parse()?,
            "Texture4" => settings.terrain_texture4 = value.parse()?,
            "PBR1" => settings.terrain_pbr1 = value.parse()?,
            "PBR2" => settings.terrain_pbr2 = value.parse()?,
            "PBR3" => settings.terrain_pbr3 = value.parse()?,
            "PBR4" => settings.terrain_pbr4 = value.parse()?,
            "ElevationLowSW" => {
                settings.elevation1_sw = value
                    .parse()
                    .map_err(|_| ArchiveError::Invalid(format!("bad ElevationLowSW: {value}")))?;
            }
            "ElevationLowNW" => {
                settings.elevation1_nw = value
                    .parse()
                    .map_err(|_| ArchiveError::Invalid(format!("bad ElevationLowNW: {value}")))?;
            }
            "ElevationLowSE" => {
                settings.elevation1_se = value
                    .parse()
                    .map_err(|_| ArchiveError::Invalid(format!("bad ElevationLowSE: {value}")))?;
            }
            "ElevationLowNE" => {
                settings.elevation1_ne = value
                    .parse()
                    .map_err(|_| ArchiveError::Invalid(format!("bad ElevationLowNE: {value}")))?;
            }
            "ElevationHighSW" => {
                settings.elevation2_sw = value
                    .parse()
                    .map_err(|_| ArchiveError::Invalid(format!("bad ElevationHighSW: {value}")))?;
            }
            "ElevationHighNW" => {
                settings.elevation2_nw = value
                    .parse()
                    .map_err(|_| ArchiveError::Invalid(format!("bad ElevationHighNW: {value}")))?;
            }
            "ElevationHighSE" => {
                settings.elevation2_se = value
                    .parse()
                    .map_err(|_| ArchiveError::Invalid(format!("bad ElevationHighSE: {value}")))?;
            }
            "ElevationHighNE" => {
                settings.elevation2_ne = value
                    .parse()
                    .map_err(|_| ArchiveError::Invalid(format!("bad ElevationHighNE: {value}")))?;
            }
            _ => {}
        },
        Section::Terrain => match name {
            "WaterHeight" => {
                settings.water_height = value
                    .parse()
                    .map_err(|_| ArchiveError::Invalid(format!("bad WaterHeight: {value}")))?;
            }
            "TerrainRaiseLimit" => {
                settings.terrain_raise_limit = value.parse().map_err(|_| {
                    ArchiveError::Invalid(format!("bad TerrainRaiseLimit: {value}"))
                })?;
            }
            "TerrainLowerLimit" => {
                settings.terrain_lower_limit = value.parse().map_err(|_| {
                    ArchiveError::Invalid(format!("bad TerrainLowerLimit: {value}"))
                })?;
            }
            "UseEstateSun" => settings.use_estate_sun = parse_bool(value),
            "FixedSun" => settings.fixed_sun = parse_bool(value),
            "SunPosition" => {
                settings.sun_position = value
                    .parse()
                    .map_err(|_| ArchiveError::Invalid(format!("bad SunPosition: {value}")))?;
            }
            _ => {}
        },
        Section::Telehub => match name {
            "TelehubObject" => settings.telehub_object = value.parse()?,
            "SpawnPoint" => settings.spawn_points.push(value.to_string()),
            _ => {}
        },
        Section::Estate => match name {
            "AllowDirectTeleport" => settings.allow_direct_teleport = parse_bool(value),
            "AllowEnvironmentOverride" => settings.allow_environment_override = parse_bool(value),
            _ => {}
        },
        Section::Root => {} // ignore elements outside known sections
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_deserialize_roundtrip() {
        let mut settings = RegionSettings::default();
        settings.allow_damage = true;
        settings.block_fly = true;
        settings.maturity = 2;
        settings.agent_limit = 100;
        settings.object_bonus = 1.5;
        settings.terrain_texture1 =
            Uuid::parse_str("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee").unwrap();
        settings.elevation1_sw = 15.5;
        settings.elevation2_ne = 80.0;
        settings.water_height = 25.0;
        settings.terrain_raise_limit = 50.0;
        settings.terrain_lower_limit = -50.0;
        settings.fixed_sun = true;
        settings.sun_position = 6.0;
        settings.telehub_object = Uuid::parse_str("11111111-2222-3333-4444-555555555555").unwrap();
        settings.spawn_points = vec!["1,2,3".to_string(), "4,5,6".to_string()];
        settings.allow_direct_teleport = false;
        settings.allow_environment_override = true;

        let xml = serialize(&settings).expect("serialize failed");
        let restored = deserialize(&xml).expect("deserialize failed");

        // General
        assert!(restored.allow_damage);
        assert!(restored.allow_land_resell);
        assert!(restored.allow_land_join_divide);
        assert!(restored.block_fly);
        assert!(!restored.block_show_in_search);
        assert!(!restored.block_terraform);
        assert!(!restored.disable_collisions);
        assert!(!restored.disable_physics);
        assert!(!restored.disable_scripts);
        assert_eq!(restored.maturity, 2);
        assert!(!restored.restrict_pushing);
        assert_eq!(restored.agent_limit, 100);
        assert!((restored.object_bonus - 1.5).abs() < f64::EPSILON);

        // GroundTextures
        assert_eq!(restored.terrain_texture1, settings.terrain_texture1);
        assert!((restored.elevation1_sw - 15.5).abs() < f64::EPSILON);
        assert!((restored.elevation2_ne - 80.0).abs() < f64::EPSILON);

        // Terrain
        assert!((restored.water_height - 25.0).abs() < f64::EPSILON);
        assert!((restored.terrain_raise_limit - 50.0).abs() < f64::EPSILON);
        assert!((restored.terrain_lower_limit - -50.0).abs() < f64::EPSILON);
        assert!(restored.fixed_sun);
        assert!((restored.sun_position - 6.0).abs() < f64::EPSILON);

        // Telehub
        assert_eq!(restored.telehub_object, settings.telehub_object);
        assert_eq!(restored.spawn_points, vec!["1,2,3", "4,5,6"]);

        // Estate
        assert!(!restored.allow_direct_teleport);
        assert!(restored.allow_environment_override);
    }

    #[test]
    fn default_values_survive_roundtrip() {
        let settings = RegionSettings::default();
        let xml = serialize(&settings).expect("serialize");
        let restored = deserialize(&xml).expect("deserialize");

        assert_eq!(restored.agent_limit, 40);
        assert!((restored.water_height - 20.0).abs() < f64::EPSILON);
        assert!((restored.terrain_raise_limit - 100.0).abs() < f64::EPSILON);
        assert!((restored.terrain_lower_limit - (-100.0)).abs() < f64::EPSILON);
        assert!(restored.use_estate_sun);
        assert_eq!(restored.telehub_object, Uuid::nil());
        assert!(restored.spawn_points.is_empty());
    }

    #[test]
    fn xml_header_present() {
        let xml = serialize(&RegionSettings::default()).unwrap();
        assert!(xml.starts_with("<?xml version=\"1.0\"?>"));
    }

    #[test]
    fn all_fields_present_in_xml() {
        let xml = serialize(&RegionSettings::default()).unwrap();
        // Spot-check key element names
        assert!(xml.contains("<AllowDamage>"));
        assert!(xml.contains("<GroundTextures>"));
        assert!(xml.contains("<WaterHeight>"));
        assert!(xml.contains("<Telehub>"));
        assert!(xml.contains("<Estate>"));
        assert!(xml.contains("<AllowDirectTeleport>"));
    }
}
