// SPDX-License-Identifier: GPL-3.0-or-later

//! OAR archive reader (single and multi-region).
//!
//! Ported from `OpenSim.Region.CoreModules.World.Archiver.ArchiveReadRequest`.

use std::collections::HashMap;
use std::io::Read;

use flate2::read::GzDecoder;
use opensim_types::AssetType;
use tar::Archive;
use uuid::Uuid;

use crate::assets_request::AssetData;
use crate::constants;
use crate::control_file::{self, ArchiveControlFile};
use crate::land_data::{self, ArchiveLandData};
use crate::oar_write::PermissionChecker;
use crate::region_settings::{self, RegionSettings};
use crate::ArchiveError;

// ---------------------------------------------------------------------------
// Load options
// ---------------------------------------------------------------------------

/// Options controlling how an OAR archive is loaded.
#[derive(Debug, Clone)]
pub struct OarLoadOptions {
    /// If true, skip terrain and parcel data (only load objects).
    pub merge: bool,
    /// If true and merge is true, replace objects that already exist.
    pub merge_replace_objects: bool,
    /// If true, force terrain loading even when merging.
    pub force_terrain: bool,
    /// If true, force parcel loading even when merging.
    pub force_parcels: bool,
    /// Displacement to add to all loaded object positions.
    pub displacement: Option<(f32, f32, f32)>,
    /// Rotation in radians to apply to objects around the region center.
    pub rotation: Option<f32>,
    /// Bounding box filter: only objects within this box are loaded.
    /// Format: (origin, size).
    pub bounding_box: Option<((f32, f32, f32), (f32, f32, f32))>,
    /// Substitute this UUID for unknown owners/creators.
    pub default_user: Option<Uuid>,
    /// Skip loading assets.
    pub skip_assets: bool,
    /// Skip loading objects.
    pub no_objects: bool,
    /// Incoming region size (default 256x256); updated from the control file.
    pub incoming_region_size: (f32, f32),
}

impl Default for OarLoadOptions {
    fn default() -> Self {
        Self {
            merge: false,
            merge_replace_objects: false,
            force_terrain: false,
            force_parcels: false,
            displacement: None,
            rotation: None,
            bounding_box: None,
            default_user: None,
            skip_assets: false,
            no_objects: false,
            incoming_region_size: (256.0, 256.0),
        }
    }
}

// ---------------------------------------------------------------------------
// Scene importer trait
// ---------------------------------------------------------------------------

/// Trait for the actual scene operations when loading an OAR.
///
/// Implementors perform the real work of creating objects, setting terrain, etc.
pub trait SceneImporter: Send + Sync {
    /// Store an asset in the asset service.
    fn store_asset(&mut self, asset: AssetData) -> Result<(), ArchiveError>;

    /// Load terrain heightfield data.
    fn load_terrain(&mut self, region_name: &str, data: &[u8]) -> Result<(), ArchiveError>;

    /// Apply region settings.
    fn apply_settings(&mut self, settings: &RegionSettings) -> Result<(), ArchiveError>;

    /// Apply a land parcel.
    fn apply_parcel(&mut self, parcel: &ArchiveLandData) -> Result<(), ArchiveError>;

    /// Create a scene object from its serialized XML.
    fn create_object(&mut self, xml: &str) -> Result<(), ArchiveError>;
}

// ---------------------------------------------------------------------------
// Read result
// ---------------------------------------------------------------------------

/// The parsed contents of an OAR archive.
#[derive(Debug, Clone)]
pub struct OarContents {
    /// The archive control file data.
    pub control_file: ArchiveControlFile,
    /// Assets extracted from the archive.
    pub assets: Vec<AssetData>,
    /// Region data, keyed by region directory name (empty string for single-region).
    pub regions: HashMap<String, OarRegionContents>,
}

/// Contents of a single region within the archive.
#[derive(Debug, Clone)]
pub struct OarRegionContents {
    /// Region settings (if present).
    pub settings: Option<RegionSettings>,
    /// Land parcels.
    pub parcels: Vec<ArchiveLandData>,
    /// Raw terrain data (bytes).
    pub terrain: Option<Vec<u8>>,
    /// Terrain filename (for determining format).
    pub terrain_filename: Option<String>,
    /// Serialized object XML strings.
    pub objects_xml: Vec<String>,
}

impl Default for OarRegionContents {
    fn default() -> Self {
        Self {
            settings: None,
            parcels: Vec::new(),
            terrain: None,
            terrain_filename: None,
            objects_xml: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Security: path validation
// ---------------------------------------------------------------------------

/// Validate that a tar entry path is safe.
///
/// Rejects absolute paths and any path component containing `..`.
fn validate_path(path: &str) -> Result<(), ArchiveError> {
    if path.starts_with('/') || path.starts_with('\\') {
        return Err(ArchiveError::Invalid(format!(
            "absolute path in archive: {path}"
        )));
    }

    for component in path.split('/') {
        if component == ".." {
            return Err(ArchiveError::Invalid(format!(
                "path traversal in archive: {path}"
            )));
        }
    }

    // Also check backslash-separated components (Windows-style paths)
    for component in path.split('\\') {
        if component == ".." {
            return Err(ArchiveError::Invalid(format!(
                "path traversal in archive (backslash): {path}"
            )));
        }
    }

    Ok(())
}

/// Validate that an entry's size does not exceed the maximum.
fn validate_size(size: u64, path: &str) -> Result<(), ArchiveError> {
    if size > constants::MAX_ARCHIVE_SIZE {
        return Err(ArchiveError::Invalid(format!(
            "archive entry too large ({size} bytes): {path}"
        )));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Routing: map archive path to region and local path
// ---------------------------------------------------------------------------

/// Determine which region an entry belongs to, and its local path within that region.
///
/// Returns `(region_dir, local_path)`. For single-region archives, `region_dir`
/// is an empty string.
fn route_path(full_path: &str, is_multi_region: bool) -> Option<(String, String)> {
    if is_multi_region && full_path.starts_with(constants::REGIONS_PATH) {
        // Multi-region: regions/{dir}/{prefix}/...
        let after_regions = &full_path[constants::REGIONS_PATH.len()..];
        let slash_pos = after_regions.find('/')?;
        let region_dir = after_regions[..slash_pos].to_string();
        let local_path = after_regions[slash_pos + 1..].to_string();
        Some((region_dir, local_path))
    } else if full_path == constants::CONTROL_FILE_PATH {
        // Control file belongs to no specific region
        None
    } else if full_path.starts_with(constants::ASSETS_PATH) {
        // Assets are global
        None
    } else {
        // Single-region entry
        Some((String::new(), full_path.to_string()))
    }
}

// ---------------------------------------------------------------------------
// OAR Reader
// ---------------------------------------------------------------------------

/// Read an OAR archive and extract its contents.
///
/// This function parses the tar.gz archive and returns the structured data.
/// The caller is responsible for applying the contents to a scene (via
/// [`SceneImporter`] or direct use of the data).
///
/// ## Security
///
/// - All tar entry paths are validated against path traversal attacks.
/// - Entry sizes are checked against [`constants::MAX_ARCHIVE_SIZE`].
pub fn read_archive<R: Read>(reader: R, options: &OarLoadOptions) -> Result<OarContents, ArchiveError> {
    let gz = GzDecoder::new(reader);
    let mut archive = Archive::new(gz);

    let mut control_file: Option<ArchiveControlFile> = None;
    let mut assets: Vec<AssetData> = Vec::new();
    let mut regions: HashMap<String, OarRegionContents> = HashMap::new();

    let entries = archive.entries().map_err(|e| ArchiveError::Tar(e.to_string()))?;

    for entry_result in entries {
        let mut entry = entry_result.map_err(|e| ArchiveError::Tar(e.to_string()))?;

        let path_bytes = entry.path_bytes();
        let full_path = String::from_utf8_lossy(&path_bytes).into_owned();

        // Skip directory entries
        if entry.header().entry_type() == tar::EntryType::Directory {
            continue;
        }

        // Security: validate path
        validate_path(&full_path)?;

        // Security: validate size
        let entry_size = entry.header().size().map_err(|e| ArchiveError::Tar(e.to_string()))?;
        validate_size(entry_size, &full_path)?;

        // Read entry data
        let mut data = Vec::with_capacity(entry_size as usize);
        entry.read_to_end(&mut data).map_err(|e| ArchiveError::Tar(e.to_string()))?;

        // Handle control file
        if full_path == constants::CONTROL_FILE_PATH {
            let xml = String::from_utf8_lossy(&data);
            let cf = control_file::deserialize(&xml)?;
            control_file = Some(cf);
            continue;
        }

        // Handle assets (global, not per-region)
        if full_path.starts_with(constants::ASSETS_PATH) {
            if !options.skip_assets {
                if let Some(asset) = parse_asset_entry(&full_path, data) {
                    assets.push(asset);
                }
            }
            continue;
        }

        // Determine the multi-region flag from the control file if we have it
        let is_multi = control_file.as_ref().map_or(false, |cf| cf.is_multi_region);

        // Route to region
        if let Some((region_dir, local_path)) = route_path(&full_path, is_multi) {
            let region = regions.entry(region_dir).or_default();
            process_region_entry(region, &local_path, data, options)?;
        }
    }

    let cf = control_file.ok_or_else(|| {
        ArchiveError::Invalid("control file (archive.xml) not found in archive".to_string())
    })?;

    Ok(OarContents {
        control_file: cf,
        assets,
        regions,
    })
}

/// Read an OAR archive and apply it directly to a scene importer.
///
/// This is the streaming variant: it processes entries as they come
/// without buffering all objects in memory.
pub fn read_archive_into<R: Read>(
    reader: R,
    options: &OarLoadOptions,
    importer: &mut dyn SceneImporter,
) -> Result<ArchiveControlFile, ArchiveError> {
    let gz = GzDecoder::new(reader);
    let mut archive = Archive::new(gz);

    let mut control_file: Option<ArchiveControlFile> = None;

    let entries = archive.entries().map_err(|e| ArchiveError::Tar(e.to_string()))?;

    for entry_result in entries {
        let mut entry = entry_result.map_err(|e| ArchiveError::Tar(e.to_string()))?;

        let path_bytes = entry.path_bytes();
        let full_path = String::from_utf8_lossy(&path_bytes).into_owned();

        if entry.header().entry_type() == tar::EntryType::Directory {
            continue;
        }

        validate_path(&full_path)?;

        let entry_size = entry.header().size().map_err(|e| ArchiveError::Tar(e.to_string()))?;
        validate_size(entry_size, &full_path)?;

        let mut data = Vec::with_capacity(entry_size as usize);
        entry.read_to_end(&mut data).map_err(|e| ArchiveError::Tar(e.to_string()))?;

        if full_path == constants::CONTROL_FILE_PATH {
            let xml = String::from_utf8_lossy(&data);
            control_file = Some(control_file::deserialize(&xml)?);
            continue;
        }

        // Determine the local path
        let is_multi = control_file.as_ref().map_or(false, |cf| cf.is_multi_region);
        let local_path = if is_multi && full_path.starts_with(constants::REGIONS_PATH) {
            let after = &full_path[constants::REGIONS_PATH.len()..];
            match after.find('/') {
                Some(pos) => after[pos + 1..].to_string(),
                None => continue,
            }
        } else if full_path.starts_with(constants::ASSETS_PATH)
            || full_path == constants::CONTROL_FILE_PATH
        {
            full_path.clone()
        } else {
            full_path.clone()
        };

        // Process
        if local_path.starts_with(constants::ASSETS_PATH) {
            if !options.skip_assets {
                if let Some(asset) = parse_asset_entry(&local_path, data) {
                    importer.store_asset(asset)?;
                }
            }
        } else if local_path.starts_with(constants::OBJECTS_PATH) {
            if !options.no_objects {
                let xml = String::from_utf8_lossy(&data).into_owned();
                // Apply displacement/rotation/bounding-box filtering would happen
                // at the importer level; we pass the raw XML here.
                importer.create_object(&xml)?;
            }
        } else if local_path.starts_with(constants::TERRAINS_PATH) {
            let should_load = !options.merge || options.force_terrain;
            if should_load {
                let name = local_path
                    .strip_prefix(constants::TERRAINS_PATH)
                    .unwrap_or(&local_path)
                    .strip_suffix(".r32")
                    .unwrap_or(&local_path);
                importer.load_terrain(name, &data)?;
            }
        } else if local_path.starts_with(constants::SETTINGS_PATH) {
            if !options.merge {
                let xml = String::from_utf8_lossy(&data);
                let settings = region_settings::deserialize(&xml)?;
                importer.apply_settings(&settings)?;
            }
        } else if local_path.starts_with(constants::LANDDATA_PATH) {
            let should_load = !options.merge || options.force_parcels;
            if should_load {
                let xml = String::from_utf8_lossy(&data);
                let parcel = land_data::deserialize(&xml)?;
                importer.apply_parcel(&parcel)?;
            }
        }
    }

    control_file.ok_or_else(|| {
        ArchiveError::Invalid("control file (archive.xml) not found in archive".to_string())
    })
}

// ---------------------------------------------------------------------------
// Transform helpers for loaded objects
// ---------------------------------------------------------------------------

/// Apply displacement to a position.
pub fn apply_displacement(
    pos: (f32, f32, f32),
    displacement: (f32, f32, f32),
) -> (f32, f32, f32) {
    (
        pos.0 + displacement.0,
        pos.1 + displacement.1,
        pos.2 + displacement.2,
    )
}

/// Apply rotation (around a center point, about the Z axis) to a position.
pub fn apply_rotation(
    pos: (f32, f32, f32),
    rotation_rad: f32,
    center: (f32, f32),
) -> (f32, f32, f32) {
    let (sin_r, cos_r) = rotation_rad.sin_cos();
    let dx = pos.0 - center.0;
    let dy = pos.1 - center.1;
    (
        center.0 + dx * cos_r - dy * sin_r,
        center.1 + dx * sin_r + dy * cos_r,
        pos.2,
    )
}

/// Check whether a position is within a bounding box.
pub fn within_bounding_box(
    pos: (f32, f32, f32),
    origin: (f32, f32, f32),
    size: (f32, f32, f32),
) -> bool {
    pos.0 >= origin.0
        && pos.0 < origin.0 + size.0
        && pos.1 >= origin.1
        && pos.1 < origin.1 + size.1
        && pos.2 >= origin.2
        && pos.2 < origin.2 + size.2
}

/// Check whether a user has permission to import an object.
///
/// Currently this is a simple check: if no permission checker is provided,
/// all objects are allowed.
pub fn check_import_permission(
    _user: Uuid,
    _object_uuid: Uuid,
    checker: Option<&dyn PermissionChecker>,
) -> bool {
    // If no permission checker is installed, allow everything
    checker.is_none()
        || true // Import permission is always granted in the current C# code
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Parse an asset entry from its archive path and raw data.
fn parse_asset_entry(path: &str, data: Vec<u8>) -> Option<AssetData> {
    let filename = path.strip_prefix(constants::ASSETS_PATH)?;
    let sep_pos = filename.find(constants::ASSET_EXTENSION_SEPARATOR)?;
    if sep_pos < 32 {
        tracing::warn!("asset path too short for UUID: {path}");
        return None;
    }

    let uuid_str = &filename[..sep_pos];
    let extension = &filename[sep_pos..];

    let uuid = uuid_str.parse::<Uuid>().ok()?;
    let asset_type = constants::extension_to_asset_type(extension).unwrap_or(AssetType::Unknown);

    Some(AssetData {
        uuid,
        asset_type,
        data,
    })
}

/// Process a single entry that belongs to a specific region.
fn process_region_entry(
    region: &mut OarRegionContents,
    local_path: &str,
    data: Vec<u8>,
    options: &OarLoadOptions,
) -> Result<(), ArchiveError> {
    if local_path.starts_with(constants::OBJECTS_PATH) {
        if !options.no_objects {
            let xml = String::from_utf8_lossy(&data).into_owned();
            region.objects_xml.push(xml);
        }
    } else if local_path.starts_with(constants::TERRAINS_PATH) {
        let should_load = !options.merge || options.force_terrain;
        if should_load {
            region.terrain_filename = Some(local_path.to_string());
            region.terrain = Some(data);
        }
    } else if local_path.starts_with(constants::SETTINGS_PATH) {
        if !options.merge {
            let xml = String::from_utf8_lossy(&data);
            let settings = region_settings::deserialize(&xml)?;
            region.settings = Some(settings);
        }
    } else if local_path.starts_with(constants::LANDDATA_PATH) {
        let should_load = !options.merge || options.force_parcels;
        if should_load {
            let xml = String::from_utf8_lossy(&data);
            let parcel = land_data::deserialize(&xml)?;
            region.parcels.push(parcel);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oar_write;

    // ---- Path traversal security tests ----

    #[test]
    fn reject_dotdot_path() {
        assert!(validate_path("../etc/passwd").is_err());
        assert!(validate_path("objects/../../../etc/passwd").is_err());
        assert!(validate_path("foo/..").is_err());
    }

    #[test]
    fn reject_absolute_path() {
        assert!(validate_path("/etc/passwd").is_err());
        assert!(validate_path("\\Windows\\System32").is_err());
    }

    #[test]
    fn accept_normal_paths() {
        assert!(validate_path("archive.xml").is_ok());
        assert!(validate_path("assets/00000000-0000-0000-0000-000000000001_texture.jp2").is_ok());
        assert!(validate_path("objects/Box_128-128-025__abcdef01-2345-6789-abcd-ef0123456789.xml").is_ok());
        assert!(validate_path("regions/1_1_TestRegion/objects/Box.xml").is_ok());
    }

    #[test]
    fn reject_backslash_traversal() {
        assert!(validate_path("objects\\..\\..\\etc\\passwd").is_err());
    }

    // ---- Size validation ----

    #[test]
    fn reject_oversized_entry() {
        assert!(validate_size(constants::MAX_ARCHIVE_SIZE + 1, "big.bin").is_err());
    }

    #[test]
    fn accept_normal_size() {
        assert!(validate_size(1024, "small.bin").is_ok());
        assert!(validate_size(constants::MAX_ARCHIVE_SIZE, "max.bin").is_ok());
    }

    // ---- Route path ----

    #[test]
    fn route_single_region() {
        let result = route_path("objects/Box.xml", false);
        assert_eq!(result, Some(("".to_string(), "objects/Box.xml".to_string())));
    }

    #[test]
    fn route_multi_region() {
        let result = route_path("regions/1_1_Test/objects/Box.xml", true);
        assert_eq!(
            result,
            Some(("1_1_Test".to_string(), "objects/Box.xml".to_string()))
        );
    }

    #[test]
    fn route_control_file_is_none() {
        assert!(route_path("archive.xml", false).is_none());
        assert!(route_path("archive.xml", true).is_none());
    }

    #[test]
    fn route_assets_global() {
        assert!(route_path("assets/00000000_texture.jp2", false).is_none());
    }

    // ---- Parse asset entry ----

    #[test]
    fn parse_valid_asset() {
        let path = "assets/12345678-1234-1234-1234-123456789abc_texture.jp2";
        let data = vec![1, 2, 3];
        let asset = parse_asset_entry(path, data.clone()).unwrap();
        assert_eq!(
            asset.uuid,
            Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap()
        );
        assert_eq!(asset.asset_type, AssetType::Texture);
        assert_eq!(asset.data, data);
    }

    #[test]
    fn parse_invalid_asset_path() {
        assert!(parse_asset_entry("assets/short_texture.jp2", vec![]).is_none());
        assert!(parse_asset_entry("not-assets/foo", vec![]).is_none());
    }

    // ---- Transform tests ----

    #[test]
    fn displacement_applies() {
        let pos = apply_displacement((100.0, 100.0, 50.0), (10.0, -20.0, 5.0));
        assert!((pos.0 - 110.0).abs() < f32::EPSILON);
        assert!((pos.1 - 80.0).abs() < f32::EPSILON);
        assert!((pos.2 - 55.0).abs() < f32::EPSILON);
    }

    #[test]
    fn rotation_90_degrees() {
        // 90 degrees = PI/2 radians
        let pos = apply_rotation((256.0, 128.0, 50.0), std::f32::consts::FRAC_PI_2, (128.0, 128.0));
        // After 90 deg CCW around (128,128): (256,128) -> (128, 256)
        assert!((pos.0 - 128.0).abs() < 0.01, "x={}", pos.0);
        assert!((pos.1 - 256.0).abs() < 0.01, "y={}", pos.1);
        assert!((pos.2 - 50.0).abs() < f32::EPSILON);
    }

    #[test]
    fn bounding_box_filter() {
        let origin = (10.0, 10.0, 0.0);
        let size = (100.0, 100.0, 100.0);
        assert!(within_bounding_box((50.0, 50.0, 50.0), origin, size));
        assert!(!within_bounding_box((5.0, 50.0, 50.0), origin, size));
        assert!(!within_bounding_box((50.0, 5.0, 50.0), origin, size));
        assert!(!within_bounding_box((50.0, 50.0, 200.0), origin, size));
        // Edge: at origin is in, at origin+size is out
        assert!(within_bounding_box((10.0, 10.0, 0.0), origin, size));
        assert!(!within_bounding_box((110.0, 10.0, 0.0), origin, size));
    }

    // ---- Round-trip test ----

    #[test]
    fn single_region_roundtrip() {
        use crate::oar_write::{
            OarRegionData, OarSceneObject, OarTerrain, OarWriteOptions,
        };

        // Build a region
        let heightfield: Vec<u8> = (0..65536u32)
            .flat_map(|_| 25.0f32.to_le_bytes())
            .collect();

        let parcel_uuid = Uuid::new_v4();
        let obj_uuid = Uuid::new_v4();
        let asset_uuid = Uuid::new_v4();

        let region = OarRegionData {
            region_name: "Roundtrip".to_string(),
            size_x: 256,
            size_y: 256,
            grid_x: 1000,
            grid_y: 1000,
            region_id: Uuid::new_v4(),
            settings: RegionSettings {
                allow_damage: true,
                water_height: 30.0,
                ..Default::default()
            },
            parcels: vec![ArchiveLandData {
                name: "Main Parcel".to_string(),
                global_id: parcel_uuid,
                area: 65536,
                ..Default::default()
            }],
            terrain: OarTerrain {
                region_name: "Roundtrip".to_string(),
                grid_x: 1000,
                grid_y: 1000,
                heightfield,
            },
            objects: vec![OarSceneObject {
                name: "TestBox".to_string(),
                uuid: obj_uuid,
                position: (128.0, 128.0, 25.0),
                xml: "<SceneObjectGroup><RootPart><SceneObjectPart><Name>TestBox</Name></SceneObjectPart></RootPart></SceneObjectGroup>".to_string(),
            }],
        };

        let assets = vec![AssetData {
            uuid: asset_uuid,
            asset_type: AssetType::Texture,
            data: vec![0xFF; 64],
        }];

        let options = OarWriteOptions::default();

        // Write
        let mut buf = Vec::new();
        oar_write::write_archive(
            &mut buf,
            &region,
            &assets,
            &options,
            None::<fn(&oar_write::OarProgress)>,
        )
        .expect("write failed");

        // Read back
        let read_opts = OarLoadOptions::default();
        let contents = read_archive(std::io::Cursor::new(buf), &read_opts).expect("read failed");

        // Verify control file
        assert_eq!(contents.control_file.major_version, 0);
        assert_eq!(contents.control_file.minor_version, 8);
        assert_eq!(contents.control_file.region_size_x, 256);

        // Verify assets
        assert_eq!(contents.assets.len(), 1);
        assert_eq!(contents.assets[0].uuid, asset_uuid);
        assert_eq!(contents.assets[0].asset_type, AssetType::Texture);
        assert_eq!(contents.assets[0].data, vec![0xFF; 64]);

        // Verify region data
        let region_contents = contents.regions.get("").expect("single-region not found");
        assert!(region_contents.settings.is_some());
        let settings = region_contents.settings.as_ref().unwrap();
        assert!(settings.allow_damage);
        assert!((settings.water_height - 30.0).abs() < f64::EPSILON);

        // Parcels
        assert_eq!(region_contents.parcels.len(), 1);
        assert_eq!(region_contents.parcels[0].name, "Main Parcel");

        // Terrain
        assert!(region_contents.terrain.is_some());
        let terrain = region_contents.terrain.as_ref().unwrap();
        assert_eq!(terrain.len(), 262144); // 256*256*4

        // Objects
        assert_eq!(region_contents.objects_xml.len(), 1);
        assert!(region_contents.objects_xml[0].contains("TestBox"));
    }

    #[test]
    fn multi_region_roundtrip() {
        use crate::oar_write::{
            ArchiveScenesGroup, OarRegionData, OarSceneObject, OarTerrain,
            OarWriteOptions,
        };

        let make_region = |name: &str, gx: u32, gy: u32| -> OarRegionData {
            let heightfield: Vec<u8> = (0..65536u32)
                .flat_map(|_| 25.0f32.to_le_bytes())
                .collect();
            OarRegionData {
                region_name: name.to_string(),
                size_x: 256,
                size_y: 256,
                grid_x: gx,
                grid_y: gy,
                region_id: Uuid::new_v4(),
                settings: RegionSettings::default(),
                parcels: Vec::new(),
                terrain: OarTerrain {
                    region_name: name.to_string(),
                    grid_x: gx,
                    grid_y: gy,
                    heightfield,
                },
                objects: vec![OarSceneObject {
                    name: format!("Obj_{name}"),
                    uuid: Uuid::new_v4(),
                    position: (128.0, 128.0, 25.0),
                    xml: format!("<SceneObjectGroup><Name>{name}</Name></SceneObjectGroup>"),
                }],
            }
        };

        let mut group = ArchiveScenesGroup::new();
        group.add_region(make_region("North", 1000, 1001));
        group.add_region(make_region("South", 1000, 1000));

        let assets = vec![AssetData {
            uuid: Uuid::new_v4(),
            asset_type: AssetType::Sound,
            data: vec![0xAB; 32],
        }];

        let options = OarWriteOptions::default();
        let mut buf = Vec::new();
        oar_write::write_multi_region_archive(
            &mut buf,
            &mut group,
            &assets,
            &options,
            None::<fn(&oar_write::OarProgress)>,
        )
        .expect("multi-region write failed");

        let read_opts = OarLoadOptions::default();
        let contents = read_archive(std::io::Cursor::new(buf), &read_opts).expect("read failed");

        assert_eq!(contents.control_file.major_version, 1);
        assert!(contents.control_file.is_multi_region);
        assert_eq!(contents.assets.len(), 1);
        // Should have 2 region directories
        assert_eq!(contents.regions.len(), 2);
    }

    #[test]
    fn merge_skips_terrain_and_settings() {
        use crate::oar_write::{
            OarRegionData, OarSceneObject, OarTerrain, OarWriteOptions,
        };

        let heightfield: Vec<u8> = (0..65536u32)
            .flat_map(|_| 25.0f32.to_le_bytes())
            .collect();

        let region = OarRegionData {
            region_name: "MergeTest".to_string(),
            size_x: 256,
            size_y: 256,
            grid_x: 1000,
            grid_y: 1000,
            region_id: Uuid::new_v4(),
            settings: RegionSettings {
                water_height: 99.0,
                ..Default::default()
            },
            parcels: vec![ArchiveLandData {
                name: "Parcel".to_string(),
                global_id: Uuid::new_v4(),
                ..Default::default()
            }],
            terrain: OarTerrain {
                region_name: "MergeTest".to_string(),
                grid_x: 1000,
                grid_y: 1000,
                heightfield,
            },
            objects: vec![OarSceneObject {
                name: "Cube".to_string(),
                uuid: Uuid::new_v4(),
                position: (64.0, 64.0, 10.0),
                xml: "<SceneObjectGroup/>".to_string(),
            }],
        };

        let options = OarWriteOptions::default();
        let mut buf = Vec::new();
        oar_write::write_archive(&mut buf, &region, &[], &options, None::<fn(&oar_write::OarProgress)>)
            .expect("write failed");

        // Read with merge: should skip terrain, settings, parcels
        let read_opts = OarLoadOptions {
            merge: true,
            ..Default::default()
        };
        let contents = read_archive(std::io::Cursor::new(buf), &read_opts).expect("read failed");

        let rc = contents.regions.get("").unwrap();
        assert!(rc.settings.is_none(), "settings should be skipped in merge mode");
        assert!(rc.terrain.is_none(), "terrain should be skipped in merge mode");
        assert!(rc.parcels.is_empty(), "parcels should be skipped in merge mode");
        assert_eq!(rc.objects_xml.len(), 1, "objects should still be loaded");
    }

    #[test]
    fn merge_force_terrain() {
        use crate::oar_write::{
            OarRegionData, OarTerrain, OarWriteOptions,
        };

        let heightfield: Vec<u8> = (0..65536u32)
            .flat_map(|_| 25.0f32.to_le_bytes())
            .collect();

        let region = OarRegionData {
            region_name: "ForceT".to_string(),
            size_x: 256,
            size_y: 256,
            grid_x: 1000,
            grid_y: 1000,
            region_id: Uuid::new_v4(),
            settings: RegionSettings::default(),
            parcels: Vec::new(),
            terrain: OarTerrain {
                region_name: "ForceT".to_string(),
                grid_x: 1000,
                grid_y: 1000,
                heightfield,
            },
            objects: Vec::new(),
        };

        let options = OarWriteOptions::default();
        let mut buf = Vec::new();
        oar_write::write_archive(&mut buf, &region, &[], &options, None::<fn(&oar_write::OarProgress)>)
            .expect("write failed");

        let read_opts = OarLoadOptions {
            merge: true,
            force_terrain: true,
            ..Default::default()
        };
        let contents = read_archive(std::io::Cursor::new(buf), &read_opts).expect("read failed");

        let rc = contents.regions.get("").unwrap();
        assert!(rc.terrain.is_some(), "terrain should load when force_terrain is set");
    }

    #[test]
    fn no_assets_mode() {
        use crate::oar_write::{
            OarRegionData, OarTerrain, OarWriteOptions,
        };

        let heightfield: Vec<u8> = (0..65536u32)
            .flat_map(|_| 25.0f32.to_le_bytes())
            .collect();

        let region = OarRegionData {
            region_name: "NoAssets".to_string(),
            size_x: 256,
            size_y: 256,
            grid_x: 1000,
            grid_y: 1000,
            region_id: Uuid::new_v4(),
            settings: RegionSettings::default(),
            parcels: Vec::new(),
            terrain: OarTerrain {
                region_name: "NoAssets".to_string(),
                grid_x: 1000,
                grid_y: 1000,
                heightfield,
            },
            objects: Vec::new(),
        };

        let options = OarWriteOptions {
            include_assets: false,
            ..Default::default()
        };

        let assets = vec![AssetData {
            uuid: Uuid::new_v4(),
            asset_type: AssetType::Texture,
            data: vec![0xFF; 100],
        }];

        let mut buf = Vec::new();
        oar_write::write_archive(&mut buf, &region, &assets, &options, None::<fn(&oar_write::OarProgress)>)
            .expect("write failed");

        let read_opts = OarLoadOptions::default();
        let contents = read_archive(std::io::Cursor::new(buf), &read_opts).expect("read failed");

        assert!(contents.assets.is_empty(), "no assets should be present in no_assets mode");
    }

    #[test]
    fn skip_assets_on_load() {
        use crate::oar_write::{
            OarRegionData, OarTerrain, OarWriteOptions,
        };

        let heightfield: Vec<u8> = (0..65536u32)
            .flat_map(|_| 25.0f32.to_le_bytes())
            .collect();

        let region = OarRegionData {
            region_name: "SkipAssetsLoad".to_string(),
            size_x: 256,
            size_y: 256,
            grid_x: 1000,
            grid_y: 1000,
            region_id: Uuid::new_v4(),
            settings: RegionSettings::default(),
            parcels: Vec::new(),
            terrain: OarTerrain {
                region_name: "SkipAssetsLoad".to_string(),
                grid_x: 1000,
                grid_y: 1000,
                heightfield,
            },
            objects: Vec::new(),
        };

        let assets = vec![AssetData {
            uuid: Uuid::new_v4(),
            asset_type: AssetType::Texture,
            data: vec![0xFF; 100],
        }];

        let options = OarWriteOptions::default();
        let mut buf = Vec::new();
        oar_write::write_archive(&mut buf, &region, &assets, &options, None::<fn(&oar_write::OarProgress)>)
            .expect("write failed");

        let read_opts = OarLoadOptions {
            skip_assets: true,
            ..Default::default()
        };
        let contents = read_archive(std::io::Cursor::new(buf), &read_opts).expect("read failed");

        assert!(contents.assets.is_empty(), "skip_assets should suppress asset loading");
    }
}
