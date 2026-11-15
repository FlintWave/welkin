// SPDX-License-Identifier: GPL-3.0-or-later

//! OAR archive writer (single and multi-region).
//!
//! Ported from `OpenSim.Region.CoreModules.World.Archiver.ArchiveWriteRequest`
//! and `ArchiveScenesGroup`.

use std::io::Write;

use flate2::write::GzEncoder;
use flate2::Compression;
use tar::{Builder, Header};
use uuid::Uuid;

use crate::assets_request::AssetData;
use crate::constants;
use crate::control_file::{self, ArchiveControlFile, ArchiveRegionInfo};
use crate::land_data::{self, ArchiveLandData};
use crate::region_settings::{self, RegionSettings};
use crate::ArchiveError;

// ---------------------------------------------------------------------------
// Progress reporting
// ---------------------------------------------------------------------------

/// Progress information for an OAR write operation.
#[derive(Debug, Clone)]
pub struct OarProgress {
    /// Phase description.
    pub phase: OarWritePhase,
    /// Items processed so far in this phase.
    pub current: usize,
    /// Total items in this phase (0 if unknown).
    pub total: usize,
}

/// Phases of an OAR write operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OarWritePhase {
    ControlFile,
    Settings,
    LandData,
    Terrain,
    Objects,
    Assets,
    Complete,
}

// ---------------------------------------------------------------------------
// Permission checking
// ---------------------------------------------------------------------------

/// Permission class of a user relative to an object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionClass {
    Owner,
    Group,
    Everyone,
}

/// Trait for checking export/import permissions on objects.
pub trait PermissionChecker: Send + Sync {
    /// Return the permission class of `user` relative to the object.
    fn get_permission_class(&self, user: Uuid, object_uuid: Uuid) -> PermissionClass;

    /// Return the base mask for the object.
    fn base_mask(&self, object_uuid: Uuid) -> u32;

    /// Return the group mask for the object.
    fn group_mask(&self, object_uuid: Uuid) -> u32;

    /// Return the everyone mask for the object.
    fn everyone_mask(&self, object_uuid: Uuid) -> u32;

    /// Return the creator UUID for the object.
    fn creator_id(&self, object_uuid: Uuid) -> Uuid;
}

/// Check whether a user can export an object, given a filter string.
///
/// `filter` contains "C" to require Copy, "T" to require Transfer.
/// `None` means no permission check (all objects allowed).
pub fn check_export_permission(
    user: Uuid,
    object_uuid: Uuid,
    filter: Option<&str>,
    checker: &dyn PermissionChecker,
) -> bool {
    let filter = match filter {
        Some(f) => f,
        None => return true,
    };

    let perm_class = checker.get_permission_class(user, object_uuid);
    let perm = match perm_class {
        PermissionClass::Owner => checker.base_mask(object_uuid),
        PermissionClass::Group => checker.group_mask(object_uuid) | checker.everyone_mask(object_uuid),
        PermissionClass::Everyone => checker.everyone_mask(object_uuid),
    };

    let can_copy = (perm & welkin_types::PermissionMask::COPY.bits()) != 0;
    let mut can_transfer = (perm & welkin_types::PermissionMask::TRANSFER.bits()) != 0;

    // Special case: if Everyone can copy then this implies it can also be transferred,
    // but only if the user is not the owner (C# behaviour).
    if perm_class != PermissionClass::Owner {
        can_transfer |= (checker.everyone_mask(object_uuid) & welkin_types::PermissionMask::COPY.bits()) != 0;
    }

    let mut permitted = true;
    if filter.contains('C') && !can_copy {
        permitted = false;
    }
    if filter.contains('T') && !can_transfer {
        permitted = false;
    }

    // Creator always gets permission
    if checker.creator_id(object_uuid) == user {
        permitted = true;
    }

    permitted
}

// ---------------------------------------------------------------------------
// Scene data for writing
// ---------------------------------------------------------------------------

/// A scene object to be written into the archive.
#[derive(Debug, Clone)]
pub struct OarSceneObject {
    /// Object name.
    pub name: String,
    /// Object UUID.
    pub uuid: Uuid,
    /// Position as (x, y, z).
    pub position: (f32, f32, f32),
    /// Serialized XML representation (xml2 format).
    pub xml: String,
}

/// Terrain data for a region.
#[derive(Debug, Clone)]
pub struct OarTerrain {
    /// Region name.
    pub region_name: String,
    /// Grid X coordinate (for multi-region path).
    pub grid_x: u32,
    /// Grid Y coordinate (for multi-region path).
    pub grid_y: u32,
    /// Raw f32 heightfield data (little-endian, 256*256 = 65536 floats for a 256m region).
    pub heightfield: Vec<u8>,
}

/// All data for a single region to be archived.
#[derive(Debug, Clone)]
pub struct OarRegionData {
    /// Region name.
    pub region_name: String,
    /// Region size in meters (X).
    pub size_x: u32,
    /// Region size in meters (Y).
    pub size_y: u32,
    /// Grid location X.
    pub grid_x: u32,
    /// Grid location Y.
    pub grid_y: u32,
    /// Region UUID.
    pub region_id: Uuid,
    /// Region settings.
    pub settings: RegionSettings,
    /// Land parcels.
    pub parcels: Vec<ArchiveLandData>,
    /// Terrain data.
    pub terrain: OarTerrain,
    /// Scene objects.
    pub objects: Vec<OarSceneObject>,
}

// ---------------------------------------------------------------------------
// OAR Writer options
// ---------------------------------------------------------------------------

/// Options for writing an OAR archive.
#[derive(Debug, Clone)]
pub struct OarWriteOptions {
    /// Whether to include asset data in the archive.
    pub include_assets: bool,
    /// Permission filter string (e.g. "CT" for copy+transfer).
    /// `None` means no permission check.
    pub filter_content: Option<String>,
    /// Whether to wipe owner IDs in land data.
    pub wipe_owners: bool,
}

impl Default for OarWriteOptions {
    fn default() -> Self {
        Self {
            include_assets: true,
            filter_content: None,
            wipe_owners: false,
        }
    }
}

// ---------------------------------------------------------------------------
// OarWriter: single region
// ---------------------------------------------------------------------------

/// Writes an OAR archive (single region) to a compressed tar stream.
pub fn write_archive<W, F>(
    writer: W,
    region: &OarRegionData,
    assets: &[AssetData],
    options: &OarWriteOptions,
    mut progress: Option<F>,
) -> Result<(), ArchiveError>
where
    W: Write,
    F: FnMut(&OarProgress),
{
    let gz = GzEncoder::new(writer, Compression::best());
    let mut tar = Builder::new(gz);

    // 1. Control file
    let control = ArchiveControlFile {
        major_version: 0,
        minor_version: 8,
        timestamp: chrono::Utc::now().timestamp(),
        region_size_x: region.size_x,
        region_size_y: region.size_y,
        is_multi_region: false,
        regions: Vec::new(),
    };
    let control_xml = control_file::serialize(&control)?;
    write_tar_entry(&mut tar, constants::CONTROL_FILE_PATH, control_xml.as_bytes())?;

    if let Some(ref mut cb) = progress {
        cb(&OarProgress { phase: OarWritePhase::ControlFile, current: 1, total: 1 });
    }

    // 2. Settings
    let settings_path = format!("{}{}.xml", constants::SETTINGS_PATH, region.region_name);
    let settings_xml = region_settings::serialize(&region.settings)?;
    write_tar_entry(&mut tar, &settings_path, settings_xml.as_bytes())?;

    if let Some(ref mut cb) = progress {
        cb(&OarProgress { phase: OarWritePhase::Settings, current: 1, total: 1 });
    }

    // 3. Land data
    let total_parcels = region.parcels.len();
    for (i, parcel) in region.parcels.iter().enumerate() {
        let parcel_path = constants::create_oar_land_data_path(parcel.global_id);
        let parcel_xml = land_data::serialize(parcel, options.wipe_owners)?;
        write_tar_entry(&mut tar, &parcel_path, parcel_xml.as_bytes())?;

        if let Some(ref mut cb) = progress {
            cb(&OarProgress { phase: OarWritePhase::LandData, current: i + 1, total: total_parcels });
        }
    }

    // 4. Terrain
    let terrain_path = format!(
        "{}{}.r32",
        constants::TERRAINS_PATH, region.region_name
    );
    write_tar_entry(&mut tar, &terrain_path, &region.terrain.heightfield)?;

    if let Some(ref mut cb) = progress {
        cb(&OarProgress { phase: OarWritePhase::Terrain, current: 1, total: 1 });
    }

    // 5. Objects
    let total_objects = region.objects.len();
    for (i, obj) in region.objects.iter().enumerate() {
        let obj_path = constants::create_oar_object_path(&obj.name, obj.uuid, obj.position);
        write_tar_entry(&mut tar, &obj_path, obj.xml.as_bytes())?;

        if let Some(ref mut cb) = progress {
            cb(&OarProgress { phase: OarWritePhase::Objects, current: i + 1, total: total_objects });
        }
    }

    // 6. Assets
    if options.include_assets {
        let total_assets = assets.len();
        for (i, asset) in assets.iter().enumerate() {
            let ext = constants::asset_type_to_extension(asset.asset_type)
                .unwrap_or("_unknown.bin");
            let asset_path = format!("{}{}{}", constants::ASSETS_PATH, asset.uuid, ext);
            write_tar_entry(&mut tar, &asset_path, &asset.data)?;

            if let Some(ref mut cb) = progress {
                cb(&OarProgress { phase: OarWritePhase::Assets, current: i + 1, total: total_assets });
            }
        }
    }

    if let Some(ref mut cb) = progress {
        cb(&OarProgress { phase: OarWritePhase::Complete, current: 0, total: 0 });
    }

    // Finalize
    let gz = tar.into_inner().map_err(ArchiveError::Io)?;
    gz.finish().map_err(ArchiveError::Io)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Multi-region writer
// ---------------------------------------------------------------------------

/// A group of regions to be written as a single multi-region OAR.
///
/// Ported from `ArchiveScenesGroup`.
#[derive(Debug, Clone)]
pub struct ArchiveScenesGroup {
    /// The regions in the group.
    pub regions: Vec<OarRegionData>,
    /// Bounding box: (min_x, min_y, max_x, max_y) in grid coordinates.
    bounding_rect: Option<(u32, u32, u32, u32)>,
    /// Computed per-region directory names.
    region_dirs: Vec<String>,
}

impl ArchiveScenesGroup {
    /// Create a new empty group.
    pub fn new() -> Self {
        Self {
            regions: Vec::new(),
            bounding_rect: None,
            region_dirs: Vec::new(),
        }
    }

    /// Add a region to the group.
    pub fn add_region(&mut self, region: OarRegionData) {
        self.regions.push(region);
        self.bounding_rect = None; // invalidate
        self.region_dirs.clear();
    }

    /// Compute region directories and bounding rectangle.
    ///
    /// Must be called after all regions are added, before writing.
    pub fn calc_locations(&mut self) {
        if self.regions.is_empty() {
            self.bounding_rect = Some((0, 0, 0, 0));
            return;
        }

        let min_x = self.regions.iter().map(|r| r.grid_x).min().unwrap();
        let min_y = self.regions.iter().map(|r| r.grid_y).min().unwrap();
        let max_x = self.regions.iter().map(|r| r.grid_x).max().unwrap();
        let max_y = self.regions.iter().map(|r| r.grid_y).max().unwrap();
        self.bounding_rect = Some((min_x, min_y, max_x, max_y));

        self.region_dirs.clear();
        for region in &self.regions {
            let dir = format!(
                "{}_{}_{}",
                region.grid_x - min_x + 1,
                region.grid_y - min_y + 1,
                region.region_name.replace(' ', "_")
            );
            self.region_dirs.push(dir);
        }
    }

    /// Get the directory name for a region at index `i`.
    pub fn get_region_dir(&self, i: usize) -> Option<&str> {
        self.region_dirs.get(i).map(|s| s.as_str())
    }
}

impl Default for ArchiveScenesGroup {
    fn default() -> Self {
        Self::new()
    }
}

/// Write a multi-region OAR archive.
///
/// Assets are deduplicated across all regions (written once in the
/// top-level `assets/` directory).
pub fn write_multi_region_archive<W, F>(
    writer: W,
    group: &mut ArchiveScenesGroup,
    assets: &[AssetData],
    options: &OarWriteOptions,
    mut progress: Option<F>,
) -> Result<(), ArchiveError>
where
    W: Write,
    F: FnMut(&OarProgress),
{
    group.calc_locations();

    let gz = GzEncoder::new(writer, Compression::best());
    let mut tar = Builder::new(gz);

    // 1. Control file (multi-region format: version 1.0)
    let control = ArchiveControlFile {
        major_version: 1,
        minor_version: 0,
        timestamp: chrono::Utc::now().timestamp(),
        region_size_x: 256,
        region_size_y: 256,
        is_multi_region: true,
        regions: group
            .regions
            .iter()
            .enumerate()
            .map(|(i, r)| ArchiveRegionInfo {
                dir: group.get_region_dir(i).unwrap_or("unknown").to_string(),
                is_megaregion: false,
                size_x: r.size_x,
                size_y: r.size_y,
            })
            .collect(),
    };
    let control_xml = control_file::serialize(&control)?;
    write_tar_entry(&mut tar, constants::CONTROL_FILE_PATH, control_xml.as_bytes())?;

    if let Some(ref mut cb) = progress {
        cb(&OarProgress { phase: OarWritePhase::ControlFile, current: 1, total: 1 });
    }

    // 2. Per-region data
    for (idx, region) in group.regions.iter().enumerate() {
        let region_dir = group.get_region_dir(idx).unwrap_or("unknown");
        let prefix = format!("{}{}/", constants::REGIONS_PATH, region_dir);

        // Settings
        let settings_path = format!("{}{}{}.xml", prefix, constants::SETTINGS_PATH, region.region_name);
        let settings_xml = region_settings::serialize(&region.settings)?;
        write_tar_entry(&mut tar, &settings_path, settings_xml.as_bytes())?;

        // Land data
        for parcel in &region.parcels {
            let parcel_path = format!(
                "{}{}",
                prefix,
                constants::create_oar_land_data_path(parcel.global_id)
            );
            let parcel_xml = land_data::serialize(parcel, options.wipe_owners)?;
            write_tar_entry(&mut tar, &parcel_path, parcel_xml.as_bytes())?;
        }

        // Terrain
        let terrain_path = format!(
            "{}{}{}.r32",
            prefix, constants::TERRAINS_PATH, region.region_name
        );
        write_tar_entry(&mut tar, &terrain_path, &region.terrain.heightfield)?;

        // Objects
        for obj in &region.objects {
            let obj_filename = constants::create_oar_object_filename(&obj.name, obj.uuid, obj.position);
            let obj_path = format!("{}{}{}", prefix, constants::OBJECTS_PATH, obj_filename);
            write_tar_entry(&mut tar, &obj_path, obj.xml.as_bytes())?;
        }
    }

    // 3. Assets (deduplicated, written once at top level)
    if options.include_assets {
        let total_assets = assets.len();
        for (i, asset) in assets.iter().enumerate() {
            let ext = constants::asset_type_to_extension(asset.asset_type)
                .unwrap_or("_unknown.bin");
            let asset_path = format!("{}{}{}", constants::ASSETS_PATH, asset.uuid, ext);
            write_tar_entry(&mut tar, &asset_path, &asset.data)?;

            if let Some(ref mut cb) = progress {
                cb(&OarProgress { phase: OarWritePhase::Assets, current: i + 1, total: total_assets });
            }
        }
    }

    if let Some(ref mut cb) = progress {
        cb(&OarProgress { phase: OarWritePhase::Complete, current: 0, total: 0 });
    }

    let gz = tar.into_inner().map_err(ArchiveError::Io)?;
    gz.finish().map_err(ArchiveError::Io)?;

    Ok(())
}

/// Write a `no_assets` archive — same structure, but without the assets/ entries.
pub fn write_archive_no_assets<W, F>(
    writer: W,
    region: &OarRegionData,
    options: &OarWriteOptions,
    progress: Option<F>,
) -> Result<(), ArchiveError>
where
    W: Write,
    F: FnMut(&OarProgress),
{
    let opts = OarWriteOptions {
        include_assets: false,
        ..options.clone()
    };
    write_archive(writer, region, &[], &opts, progress)
}

// ---------------------------------------------------------------------------
// Tar helper
// ---------------------------------------------------------------------------

fn write_tar_entry<W: Write>(
    tar: &mut Builder<W>,
    path: &str,
    data: &[u8],
) -> Result<(), ArchiveError> {
    let mut header = Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    header.set_mtime(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    header.set_cksum();

    tar.append_data(&mut header, path, data)
        .map_err(|e| ArchiveError::Tar(e.to_string()))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use welkin_types::AssetType;

    fn make_test_region() -> OarRegionData {
        // 256x256 terrain = 65536 f32 = 262144 bytes
        let heightfield: Vec<u8> = (0..65536u32)
            .flat_map(|i| {
                let h = 25.0f32 + (i as f32 * 0.0001);
                h.to_le_bytes()
            })
            .collect();

        OarRegionData {
            region_name: "TestRegion".to_string(),
            size_x: 256,
            size_y: 256,
            grid_x: 1000,
            grid_y: 1000,
            region_id: Uuid::new_v4(),
            settings: RegionSettings::default(),
            parcels: vec![ArchiveLandData {
                name: "Test Parcel".to_string(),
                global_id: Uuid::new_v4(),
                ..Default::default()
            }],
            terrain: OarTerrain {
                region_name: "TestRegion".to_string(),
                grid_x: 1000,
                grid_y: 1000,
                heightfield,
            },
            objects: vec![OarSceneObject {
                name: "TestPrim".to_string(),
                uuid: Uuid::new_v4(),
                position: (128.0, 128.0, 25.0),
                xml: "<SceneObjectGroup><SceneObjectPart/></SceneObjectGroup>".to_string(),
            }],
        }
    }

    fn make_test_assets() -> Vec<AssetData> {
        vec![
            AssetData {
                uuid: Uuid::new_v4(),
                asset_type: AssetType::Texture,
                data: vec![0xFF; 100],
            },
            AssetData {
                uuid: Uuid::new_v4(),
                asset_type: AssetType::Sound,
                data: vec![0xAB; 50],
            },
        ]
    }

    #[test]
    fn write_single_region_archive() {
        let region = make_test_region();
        let assets = make_test_assets();
        let options = OarWriteOptions::default();

        let mut buf = Vec::new();
        write_archive(&mut buf, &region, &assets, &options, None::<fn(&OarProgress)>)
            .expect("write_archive failed");

        // Should produce a non-empty gzip stream
        assert!(!buf.is_empty());
        // gzip magic bytes
        assert_eq!(buf[0], 0x1f);
        assert_eq!(buf[1], 0x8b);
    }

    #[test]
    fn write_no_assets() {
        let region = make_test_region();
        let options = OarWriteOptions::default();

        let mut buf = Vec::new();
        write_archive_no_assets(&mut buf, &region, &options, None::<fn(&OarProgress)>)
            .expect("write_archive_no_assets failed");

        assert!(!buf.is_empty());
    }

    #[test]
    fn write_multi_region_oar() {
        let region1 = make_test_region();
        let mut region2 = make_test_region();
        region2.region_name = "Region2".to_string();
        region2.grid_x = 1001;
        region2.terrain.region_name = "Region2".to_string();

        let mut group = ArchiveScenesGroup::new();
        group.add_region(region1);
        group.add_region(region2);

        let assets = make_test_assets();
        let options = OarWriteOptions::default();

        let mut buf = Vec::new();
        write_multi_region_archive(&mut buf, &mut group, &assets, &options, None::<fn(&OarProgress)>)
            .expect("write_multi_region_archive failed");

        assert!(!buf.is_empty());
        assert_eq!(buf[0], 0x1f);
        assert_eq!(buf[1], 0x8b);
    }

    #[test]
    fn progress_callback_fires() {
        let region = make_test_region();
        let assets = make_test_assets();
        let options = OarWriteOptions::default();
        let phases = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let phases_clone = phases.clone();

        let mut buf = Vec::new();
        write_archive(&mut buf, &region, &assets, &options, Some(move |p: &OarProgress| {
            phases_clone.lock().unwrap().push(p.phase.clone());
        }))
        .expect("write failed");

        let phases = phases.lock().unwrap();
        assert!(phases.contains(&OarWritePhase::ControlFile));
        assert!(phases.contains(&OarWritePhase::Settings));
        assert!(phases.contains(&OarWritePhase::Terrain));
        assert!(phases.contains(&OarWritePhase::Objects));
        assert!(phases.contains(&OarWritePhase::Assets));
        assert!(phases.contains(&OarWritePhase::Complete));
    }

    #[test]
    fn scenes_group_calc_locations() {
        let region1 = OarRegionData {
            region_name: "Region One".to_string(),
            size_x: 256,
            size_y: 256,
            grid_x: 1000,
            grid_y: 2000,
            region_id: Uuid::new_v4(),
            settings: RegionSettings::default(),
            parcels: Vec::new(),
            terrain: OarTerrain {
                region_name: "Region One".to_string(),
                grid_x: 1000,
                grid_y: 2000,
                heightfield: Vec::new(),
            },
            objects: Vec::new(),
        };
        let region2 = OarRegionData {
            region_name: "Region Two".to_string(),
            size_x: 256,
            size_y: 256,
            grid_x: 1001,
            grid_y: 2001,
            region_id: Uuid::new_v4(),
            settings: RegionSettings::default(),
            parcels: Vec::new(),
            terrain: OarTerrain {
                region_name: "Region Two".to_string(),
                grid_x: 1001,
                grid_y: 2001,
                heightfield: Vec::new(),
            },
            objects: Vec::new(),
        };

        let mut group = ArchiveScenesGroup::new();
        group.add_region(region1);
        group.add_region(region2);
        group.calc_locations();

        assert_eq!(group.get_region_dir(0), Some("1_1_Region_One"));
        assert_eq!(group.get_region_dir(1), Some("2_2_Region_Two"));
    }

    #[test]
    fn export_permission_no_filter() {
        // No filter means always allowed
        assert!(check_export_permission(
            Uuid::new_v4(),
            Uuid::new_v4(),
            None,
            &NoopPermissionChecker,
        ));
    }

    #[test]
    fn export_permission_creator_bypass() {
        let user = Uuid::new_v4();
        let obj = Uuid::new_v4();

        struct CreatorChecker {
            user: Uuid,
        }
        impl PermissionChecker for CreatorChecker {
            fn get_permission_class(&self, _user: Uuid, _object_uuid: Uuid) -> PermissionClass {
                PermissionClass::Everyone
            }
            fn base_mask(&self, _object_uuid: Uuid) -> u32 { 0 }
            fn group_mask(&self, _object_uuid: Uuid) -> u32 { 0 }
            fn everyone_mask(&self, _object_uuid: Uuid) -> u32 { 0 }
            fn creator_id(&self, _object_uuid: Uuid) -> Uuid { self.user }
        }

        let checker = CreatorChecker { user };
        // Even with CT filter and no permissions, creator bypass succeeds
        assert!(check_export_permission(user, obj, Some("CT"), &checker));
    }

    /// A no-op permission checker for testing.
    struct NoopPermissionChecker;

    impl PermissionChecker for NoopPermissionChecker {
        fn get_permission_class(&self, _user: Uuid, _object_uuid: Uuid) -> PermissionClass {
            PermissionClass::Everyone
        }
        fn base_mask(&self, _object_uuid: Uuid) -> u32 { 0 }
        fn group_mask(&self, _object_uuid: Uuid) -> u32 { 0 }
        fn everyone_mask(&self, _object_uuid: Uuid) -> u32 { 0 }
        fn creator_id(&self, _object_uuid: Uuid) -> Uuid { Uuid::nil() }
    }
}
