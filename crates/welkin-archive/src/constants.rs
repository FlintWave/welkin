// SPDX-License-Identifier: GPL-3.0-or-later

//! Archive path constants and asset-type extension mappings.
//!
//! Ported from `OpenSim.Framework.Serialization.ArchiveConstants`.

use uuid::Uuid;
use welkin_types::AssetType;

/// Location of the archive control file.
pub const CONTROL_FILE_PATH: &str = "archive.xml";

/// Path prefix for assets held in an archive.
pub const ASSETS_PATH: &str = "assets/";

/// Path prefix for inventory data.
pub const INVENTORY_PATH: &str = "inventory/";

/// Path prefix for regions in a multi-region archive.
pub const REGIONS_PATH: &str = "regions/";

/// Path prefix for scene objects.
pub const OBJECTS_PATH: &str = "objects/";

/// Path prefix for terrain files.
pub const TERRAINS_PATH: &str = "terrains/";

/// Path prefix for region settings.
pub const SETTINGS_PATH: &str = "settings/";

/// Path prefix for land data.
pub const LANDDATA_PATH: &str = "landdata/";

/// Path prefix for user profiles.
pub const USERS_PATH: &str = "userprofiles/";

/// Separator between UUID and extension in archived asset filenames.
pub const ASSET_EXTENSION_SEPARATOR: &str = "_";

/// Separator between name and UUID components in inventory node names.
pub const INVENTORY_NODE_NAME_COMPONENT_SEPARATOR: &str = "__";

/// Format pattern for OAR object filenames.
///
/// `{name}_{x:03.0}-{y:03.0}-{z:03.0}__{uuid}.xml`
pub const OAR_OBJECT_FILENAME_TEMPLATE: &str = "{name}_{x:#000}-{y:#000}-{z:#000}__{uuid}.xml";

/// Safety limit: 50 GiB maximum archive size.
pub const MAX_ARCHIVE_SIZE: u64 = 50 * 1024 * 1024 * 1024;

/// Map an `AssetType` to its archive file extension (including the leading `_`
/// separator).
///
/// Returns `None` for asset types that have no archive extension mapping.
pub fn asset_type_to_extension(asset_type: AssetType) -> Option<&'static str> {
    match asset_type {
        AssetType::Animation => Some("_animation.bvh"),
        AssetType::Bodypart => Some("_bodypart.txt"),
        AssetType::CallingCard => Some("_callingcard.txt"),
        AssetType::Clothing => Some("_clothing.txt"),
        AssetType::Folder => Some("_folder.txt"),
        AssetType::Gesture => Some("_gesture.txt"),
        AssetType::ImageJPEG => Some("_image.jpg"),
        AssetType::ImageTGA => Some("_image.tga"),
        AssetType::Landmark => Some("_landmark.txt"),
        AssetType::LSLBytecode => Some("_bytecode.lso"),
        AssetType::LSLText => Some("_script.lsl"),
        AssetType::Mesh => Some("_mesh.llmesh"),
        AssetType::Notecard => Some("_notecard.txt"),
        AssetType::Object => Some("_object.xml"),
        AssetType::Simstate => Some("_simstate.bin"),
        AssetType::Sound => Some("_sound.ogg"),
        AssetType::SoundWAV => Some("_sound.wav"),
        AssetType::Texture => Some("_texture.jp2"),
        AssetType::TextureTGA => Some("_texture.tga"),
        AssetType::Settings => Some("_settings.bin"),
        AssetType::Material => Some("_prmat.dat"),
        _ => None,
    }
}

/// Reverse-lookup: given an extension string (including the leading `_`),
/// return the corresponding `AssetType`.
pub fn extension_to_asset_type(ext: &str) -> Option<AssetType> {
    match ext {
        "_animation.bvh" => Some(AssetType::Animation),
        "_bodypart.txt" => Some(AssetType::Bodypart),
        "_callingcard.txt" => Some(AssetType::CallingCard),
        "_clothing.txt" => Some(AssetType::Clothing),
        "_folder.txt" => Some(AssetType::Folder),
        "_gesture.txt" => Some(AssetType::Gesture),
        "_image.jpg" => Some(AssetType::ImageJPEG),
        "_image.tga" => Some(AssetType::ImageTGA),
        "_landmark.txt" => Some(AssetType::Landmark),
        "_bytecode.lso" => Some(AssetType::LSLBytecode),
        "_script.lsl" => Some(AssetType::LSLText),
        "_mesh.llmesh" => Some(AssetType::Mesh),
        "_notecard.txt" => Some(AssetType::Notecard),
        "_object.xml" => Some(AssetType::Object),
        "_simstate.bin" => Some(AssetType::Simstate),
        "_sound.ogg" => Some(AssetType::Sound),
        "_sound.wav" => Some(AssetType::SoundWAV),
        "_texture.jp2" => Some(AssetType::Texture),
        "_texture.tga" => Some(AssetType::TextureTGA),
        "_settings.bin" => Some(AssetType::Settings),
        "_prmat.dat" => Some(AssetType::Material),
        _ => None,
    }
}

/// Create an OAR object filename.
///
/// Format: `{name}_{x:03.0}-{y:03.0}-{z:03.0}__{uuid}.xml`
///
/// Coordinates are rounded to the nearest integer for the filename, matching
/// the C# `MathF.Round` behaviour.
pub fn create_oar_object_filename(name: &str, uuid: Uuid, pos: (f32, f32, f32)) -> String {
    format!(
        "{}_{:03.0}-{:03.0}-{:03.0}__{}.xml",
        name,
        pos.0.round(),
        pos.1.round(),
        pos.2.round(),
        uuid,
    )
}

/// Create the full archive path for an OAR object.
///
/// Prepends [`OBJECTS_PATH`] to the filename produced by
/// [`create_oar_object_filename`].
pub fn create_oar_object_path(name: &str, uuid: Uuid, pos: (f32, f32, f32)) -> String {
    format!(
        "{}{}",
        OBJECTS_PATH,
        create_oar_object_filename(name, uuid, pos)
    )
}

/// Create the archive path for a land-data parcel.
///
/// Format: `landdata/{global_id}.xml`
pub fn create_oar_land_data_path(global_id: Uuid) -> String {
    format!("{}{}.xml", LANDDATA_PATH, global_id)
}

/// Extract a plain (human-readable) path from an IAR inventory path by
/// stripping the `__uuid` component separators.
///
/// Each path component of the form `Name__uuid` is reduced to just `Name`.
/// Components that do not contain the separator are kept as-is.
pub fn extract_plain_path_from_iar_path(iar_path: &str) -> String {
    let mut plain_dirs: Vec<&str> = Vec::new();

    for component in iar_path.split('/').filter(|s| !s.is_empty()) {
        if let Some(i) = component.rfind(INVENTORY_NODE_NAME_COMPONENT_SEPARATOR) {
            plain_dirs.push(&component[..i]);
        } else {
            plain_dirs.push(component);
        }
    }

    plain_dirs.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_type_roundtrip() {
        let types = [
            AssetType::Animation,
            AssetType::Bodypart,
            AssetType::CallingCard,
            AssetType::Clothing,
            AssetType::Folder,
            AssetType::Gesture,
            AssetType::ImageJPEG,
            AssetType::ImageTGA,
            AssetType::Landmark,
            AssetType::LSLBytecode,
            AssetType::LSLText,
            AssetType::Mesh,
            AssetType::Notecard,
            AssetType::Object,
            AssetType::Simstate,
            AssetType::Sound,
            AssetType::SoundWAV,
            AssetType::Texture,
            AssetType::TextureTGA,
            AssetType::Settings,
            AssetType::Material,
        ];
        for t in types {
            let ext = asset_type_to_extension(t).unwrap();
            let back = extension_to_asset_type(ext).unwrap();
            assert_eq!(back, t, "round-trip failed for {t:?} -> {ext}");
        }
    }

    #[test]
    fn unknown_asset_type_returns_none() {
        assert!(asset_type_to_extension(AssetType::Unknown).is_none());
        assert!(asset_type_to_extension(AssetType::Link).is_none());
        assert!(asset_type_to_extension(AssetType::LinkFolder).is_none());
    }

    #[test]
    fn unknown_extension_returns_none() {
        assert!(extension_to_asset_type("_bogus.xyz").is_none());
        assert!(extension_to_asset_type("").is_none());
    }

    #[test]
    fn oar_object_filename_format() {
        let uuid = Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap();
        let name = "MyPrim";
        let pos = (128.7_f32, 64.2_f32, 22.9_f32);
        let filename = create_oar_object_filename(name, uuid, pos);
        assert_eq!(
            filename,
            "MyPrim_129-064-023__12345678-1234-1234-1234-123456789abc.xml"
        );
    }

    #[test]
    fn oar_object_path_prepends_objects() {
        let uuid = Uuid::parse_str("abcdef01-2345-6789-abcd-ef0123456789").unwrap();
        let path = create_oar_object_path("Box", uuid, (10.0, 20.0, 30.0));
        assert!(path.starts_with(OBJECTS_PATH));
        assert!(path.ends_with(".xml"));
    }

    #[test]
    fn oar_land_data_path() {
        let uuid = Uuid::parse_str("abcdef01-2345-6789-abcd-ef0123456789").unwrap();
        let path = create_oar_land_data_path(uuid);
        assert_eq!(path, "landdata/abcdef01-2345-6789-abcd-ef0123456789.xml");
    }

    #[test]
    fn iar_path_extraction() {
        let iar = "Objects__a1b2c3d4-0000-0000-0000-000000000000/MyItem__e5f6a7b8-0000-0000-0000-000000000000";
        let plain = extract_plain_path_from_iar_path(iar);
        assert_eq!(plain, "Objects/MyItem");
    }

    #[test]
    fn iar_path_no_separator() {
        let iar = "plain/path/here";
        let plain = extract_plain_path_from_iar_path(iar);
        assert_eq!(plain, "plain/path/here");
    }

    #[test]
    fn iar_path_empty() {
        assert_eq!(extract_plain_path_from_iar_path(""), "");
    }
}
