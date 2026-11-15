// SPDX-License-Identifier: GPL-3.0-or-later

//! OAR archive control file (`archive.xml`) serialization.
//!
//! Ported from the control-file handling in
//! `ArchiveWriteRequest.CreateControlFile` and `ArchiveReadRequest`.

use quick_xml::escape::unescape;
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use std::io::Cursor;

use crate::ArchiveError;

/// The archive control file (`archive.xml`).
#[derive(Debug, Clone)]
pub struct ArchiveControlFile {
    /// OAR format major version (typically 0).
    pub major_version: i32,
    /// OAR format minor version (typically 8).
    pub minor_version: i32,
    /// Unix timestamp of archive creation.
    pub timestamp: i64,
    /// Default region X size in meters.
    pub region_size_x: u32,
    /// Default region Y size in meters.
    pub region_size_y: u32,
    /// Whether this is a multi-region archive.
    pub is_multi_region: bool,
    /// Region entries (populated for multi-region archives).
    pub regions: Vec<ArchiveRegionInfo>,
}

impl Default for ArchiveControlFile {
    fn default() -> Self {
        Self {
            major_version: 0,
            minor_version: 8,
            timestamp: 0,
            region_size_x: 256,
            region_size_y: 256,
            is_multi_region: false,
            regions: Vec::new(),
        }
    }
}

/// Information about a single region in a multi-region archive.
#[derive(Debug, Clone)]
pub struct ArchiveRegionInfo {
    /// Directory name for this region within the archive.
    pub dir: String,
    /// Whether the region is a megaregion.
    pub is_megaregion: bool,
    /// Region X size in meters.
    pub size_x: u32,
    /// Region Y size in meters.
    pub size_y: u32,
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

/// Serialize the archive control file to XML.
///
/// Produces indented XML matching the C# `ArchiveWriteRequest.CreateControlFile`
/// output format.
pub fn serialize(control: &ArchiveControlFile) -> Result<String, ArchiveError> {
    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);

    // XML declaration
    writer
        .write_event(Event::Decl(quick_xml::events::BytesDecl::new(
            "1.0", None, None,
        )))
        .map_err(|e| ArchiveError::Xml(e.to_string()))?;

    // <archive major_version="0" minor_version="8">
    let mut archive_start = BytesStart::new("archive");
    archive_start.push_attribute(("major_version", control.major_version.to_string().as_str()));
    archive_start.push_attribute(("minor_version", control.minor_version.to_string().as_str()));
    writer
        .write_event(Event::Start(archive_start))
        .map_err(|e| ArchiveError::Xml(e.to_string()))?;

    // <creation_info>
    writer
        .write_event(Event::Start(BytesStart::new("creation_info")))
        .map_err(|e| ArchiveError::Xml(e.to_string()))?;
    write_element(&mut writer, "datetime", &control.timestamp.to_string())?;
    writer
        .write_event(Event::End(BytesEnd::new("creation_info")))
        .map_err(|e| ArchiveError::Xml(e.to_string()))?;

    write_element(&mut writer, "assets_included", "True")?;

    if control.is_multi_region {
        // Write multi-region manifest
        writer
            .write_event(Event::Start(BytesStart::new("regions")))
            .map_err(|e| ArchiveError::Xml(e.to_string()))?;

        // Each region gets wrapped in a <row><region>...</region></row>
        // for simplicity we put each region in its own row.
        for region in &control.regions {
            writer
                .write_event(Event::Start(BytesStart::new("row")))
                .map_err(|e| ArchiveError::Xml(e.to_string()))?;
            writer
                .write_event(Event::Start(BytesStart::new("region")))
                .map_err(|e| ArchiveError::Xml(e.to_string()))?;
            write_element(&mut writer, "dir", &region.dir)?;
            write_element(
                &mut writer,
                "size_in_meters",
                &format!("{},{}", region.size_x, region.size_y),
            )?;
            writer
                .write_event(Event::End(BytesEnd::new("region")))
                .map_err(|e| ArchiveError::Xml(e.to_string()))?;
            writer
                .write_event(Event::End(BytesEnd::new("row")))
                .map_err(|e| ArchiveError::Xml(e.to_string()))?;
        }

        writer
            .write_event(Event::End(BytesEnd::new("regions")))
            .map_err(|e| ArchiveError::Xml(e.to_string()))?;
    } else {
        // Single-region: write region_info
        writer
            .write_event(Event::Start(BytesStart::new("region_info")))
            .map_err(|e| ArchiveError::Xml(e.to_string()))?;
        write_element(
            &mut writer,
            "size_in_meters",
            &format!("{},{}", control.region_size_x, control.region_size_y),
        )?;
        writer
            .write_event(Event::End(BytesEnd::new("region_info")))
            .map_err(|e| ArchiveError::Xml(e.to_string()))?;
    }

    // </archive>
    writer
        .write_event(Event::End(BytesEnd::new("archive")))
        .map_err(|e| ArchiveError::Xml(e.to_string()))?;

    let buf = writer.into_inner().into_inner();
    String::from_utf8(buf).map_err(|e| ArchiveError::Xml(e.to_string()))
}

// ---------------------------------------------------------------------------
// Deserialization
// ---------------------------------------------------------------------------

/// Deserialize an archive control file from XML.
pub fn deserialize(xml: &str) -> Result<ArchiveControlFile, ArchiveError> {
    let mut control = ArchiveControlFile::default();
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut multi_region = false;
    let mut current_region: Option<ArchiveRegionInfo> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = std::str::from_utf8(e.name().into_inner())
                    .map_err(|e| ArchiveError::Xml(e.to_string()))?;
                match name {
                    "archive" => {
                        for attr in e.attributes().flatten() {
                            let key = std::str::from_utf8(attr.key.as_ref())
                                .map_err(|e| ArchiveError::Xml(e.to_string()))?;
                            let val = std::str::from_utf8(&attr.value)
                                .map_err(|e| ArchiveError::Xml(e.to_string()))?;
                            match key {
                                "major_version" => {
                                    control.major_version = val.parse().map_err(|_| {
                                        ArchiveError::Invalid(format!("bad major_version: {val}"))
                                    })?;
                                }
                                "minor_version" => {
                                    control.minor_version = val.parse().map_err(|_| {
                                        ArchiveError::Invalid(format!("bad minor_version: {val}"))
                                    })?;
                                }
                                _ => {}
                            }
                        }
                    }
                    "row" => {
                        multi_region = true;
                    }
                    "region" => {
                        current_region = Some(ArchiveRegionInfo {
                            dir: String::new(),
                            is_megaregion: false,
                            size_x: 256,
                            size_y: 256,
                        });
                    }
                    "datetime" | "dir" | "size_in_meters" | "id" | "assets_included" => {
                        let raw = reader
                            .read_text(e.name())
                            .map_err(|e| ArchiveError::Xml(e.to_string()))?;
                        let text = unescape(&raw).map_err(|e| ArchiveError::Xml(e.to_string()))?;
                        match name {
                            "datetime" => {
                                if let Ok(v) = text.parse::<i64>() {
                                    control.timestamp = v;
                                }
                            }
                            "dir" => {
                                if let Some(ref mut r) = current_region {
                                    r.dir = text.to_string();
                                }
                            }
                            "size_in_meters" => {
                                // Format: "x,y"
                                if let Some((sx, sy)) = parse_size(&text) {
                                    if let Some(ref mut r) = current_region {
                                        r.size_x = sx;
                                        r.size_y = sy;
                                    } else {
                                        control.region_size_x = sx;
                                        control.region_size_y = sy;
                                    }
                                }
                            }
                            _ => {} // id, assets_included — noted but not stored
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let name = std::str::from_utf8(e.name().into_inner())
                    .map_err(|e| ArchiveError::Xml(e.to_string()))?;
                match name {
                    "region" => {
                        if let Some(region) = current_region.take() {
                            control.regions.push(region);
                        }
                    }
                    "archive" => break,
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                // Handle self-closing elements like <region />
                let name = std::str::from_utf8(e.name().into_inner())
                    .map_err(|e| ArchiveError::Xml(e.to_string()))?;
                if name == "row" {
                    multi_region = true;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(ArchiveError::Xml(e.to_string())),
            _ => {}
        }
        buf.clear();
    }

    control.is_multi_region = multi_region;

    Ok(control)
}

/// Parse `"x,y"` size string.
fn parse_size(s: &str) -> Option<(u32, u32)> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() == 2 {
        // The values may be floats from C# (e.g. "256,256")
        let x: f64 = parts[0].trim().parse().ok()?;
        let y: f64 = parts[1].trim().parse().ok()?;
        Some((x as u32, y as u32))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_region_roundtrip() {
        let control = ArchiveControlFile {
            major_version: 0,
            minor_version: 8,
            timestamp: 1700000000,
            region_size_x: 256,
            region_size_y: 256,
            is_multi_region: false,
            regions: Vec::new(),
        };

        let xml = serialize(&control).expect("serialize failed");
        let restored = deserialize(&xml).expect("deserialize failed");

        assert_eq!(restored.major_version, 0);
        assert_eq!(restored.minor_version, 8);
        assert_eq!(restored.timestamp, 1700000000);
        assert_eq!(restored.region_size_x, 256);
        assert_eq!(restored.region_size_y, 256);
        assert!(!restored.is_multi_region);
        assert!(restored.regions.is_empty());
    }

    #[test]
    fn multi_region_roundtrip() {
        let control = ArchiveControlFile {
            major_version: 1,
            minor_version: 0,
            timestamp: 1700000000,
            region_size_x: 256,
            region_size_y: 256,
            is_multi_region: true,
            regions: vec![
                ArchiveRegionInfo {
                    dir: "region1".to_string(),
                    is_megaregion: false,
                    size_x: 256,
                    size_y: 256,
                },
                ArchiveRegionInfo {
                    dir: "region2".to_string(),
                    is_megaregion: false,
                    size_x: 512,
                    size_y: 512,
                },
            ],
        };

        let xml = serialize(&control).expect("serialize failed");
        let restored = deserialize(&xml).expect("deserialize failed");

        assert_eq!(restored.major_version, 1);
        assert_eq!(restored.minor_version, 0);
        assert!(restored.is_multi_region);
        assert_eq!(restored.regions.len(), 2);
        assert_eq!(restored.regions[0].dir, "region1");
        assert_eq!(restored.regions[0].size_x, 256);
        assert_eq!(restored.regions[0].size_y, 256);
        assert_eq!(restored.regions[1].dir, "region2");
        assert_eq!(restored.regions[1].size_x, 512);
        assert_eq!(restored.regions[1].size_y, 512);
    }

    #[test]
    fn version_attributes_parsed() {
        let xml = r#"<?xml version="1.0"?>
<archive major_version="0" minor_version="8">
  <creation_info>
    <datetime>1234567890</datetime>
  </creation_info>
  <assets_included>True</assets_included>
  <region_info>
    <size_in_meters>256,256</size_in_meters>
  </region_info>
</archive>"#;
        let control = deserialize(xml).expect("deserialize");
        assert_eq!(control.major_version, 0);
        assert_eq!(control.minor_version, 8);
        assert_eq!(control.timestamp, 1234567890);
        assert_eq!(control.region_size_x, 256);
        assert_eq!(control.region_size_y, 256);
    }

    #[test]
    fn default_values() {
        let control = ArchiveControlFile::default();
        assert_eq!(control.major_version, 0);
        assert_eq!(control.minor_version, 8);
        assert_eq!(control.region_size_x, 256);
        assert_eq!(control.region_size_y, 256);
        assert!(!control.is_multi_region);
    }

    #[test]
    fn xml_has_archive_root() {
        let xml = serialize(&ArchiveControlFile::default()).unwrap();
        assert!(xml.contains("<archive"));
        assert!(xml.contains("major_version="));
        assert!(xml.contains("minor_version="));
        assert!(xml.contains("</archive>"));
    }
}
