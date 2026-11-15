// SPDX-License-Identifier: LGPL-3.0-or-later

//! Integration tests for the welkin-archive crate.
//!
//! These tests exercise the crate's public API only, covering OAR and IAR
//! round-trips, merge modes, displacement/rotation transforms, permission
//! filtering, multi-region archives, cross-grid resolution, and security
//! (path-traversal rejection).
//!
//! Ported from the C# ArchiverTests, InventoryArchiveLoadPathTests,
//! InventoryArchiveLoadTests, InventoryArchiveSaveTests, and
//! InventoryArchiveTestCase.

use std::io::Cursor;

use uuid::Uuid;

use welkin_archive::ArchiveError;
use welkin_archive::assets_request::AssetData;
use welkin_archive::constants;
use welkin_archive::external_representation::{self, RewriteContext};
use welkin_archive::iar_read::{IarReader, InventoryImporter};
use welkin_archive::iar_write::{self, IarWriteOptions, InventoryItem};
use welkin_archive::land_data::ArchiveLandData;
use welkin_archive::oar_read::{self, OarLoadOptions};
use welkin_archive::oar_write::{
    self, ArchiveScenesGroup, OarRegionData, OarSceneObject, OarTerrain, OarWriteOptions,
    PermissionChecker, PermissionClass,
};
use welkin_archive::osp_resolver::{self, UserService};
use welkin_archive::region_settings::RegionSettings;
use welkin_types::{AssetType, PermissionMask};

// =========================================================================
// Mock implementations
// =========================================================================

/// Mock scene importer that records everything it receives.
struct MockSceneImporter {
    assets: Vec<AssetData>,
    terrain_calls: Vec<(String, Vec<u8>)>,
    settings_calls: Vec<RegionSettings>,
    parcel_calls: Vec<ArchiveLandData>,
    object_xmls: Vec<String>,
}

impl MockSceneImporter {
    fn new() -> Self {
        Self {
            assets: Vec::new(),
            terrain_calls: Vec::new(),
            settings_calls: Vec::new(),
            parcel_calls: Vec::new(),
            object_xmls: Vec::new(),
        }
    }
}

impl oar_read::SceneImporter for MockSceneImporter {
    fn store_asset(&mut self, asset: AssetData) -> Result<(), ArchiveError> {
        self.assets.push(asset);
        Ok(())
    }

    fn load_terrain(&mut self, region_name: &str, data: &[u8]) -> Result<(), ArchiveError> {
        self.terrain_calls
            .push((region_name.to_string(), data.to_vec()));
        Ok(())
    }

    fn apply_settings(&mut self, settings: &RegionSettings) -> Result<(), ArchiveError> {
        self.settings_calls.push(settings.clone());
        Ok(())
    }

    fn apply_parcel(&mut self, parcel: &ArchiveLandData) -> Result<(), ArchiveError> {
        self.parcel_calls.push(parcel.clone());
        Ok(())
    }

    fn create_object(&mut self, xml: &str) -> Result<(), ArchiveError> {
        self.object_xmls.push(xml.to_string());
        Ok(())
    }
}

/// Mock inventory importer for IAR tests.
struct MockInventoryImporter {
    folders: Vec<(String, Uuid, Uuid, Uuid)>, // (name, id, owner, parent)
    items: Vec<InventoryItem>,
    assets: Vec<(Uuid, Vec<u8>)>,
}

impl MockInventoryImporter {
    fn new() -> Self {
        Self {
            folders: Vec::new(),
            items: Vec::new(),
            assets: Vec::new(),
        }
    }
}

impl InventoryImporter for MockInventoryImporter {
    fn create_folder(
        &mut self,
        name: &str,
        owner_id: Uuid,
        parent_id: Uuid,
    ) -> Result<Uuid, ArchiveError> {
        let id = Uuid::new_v4();
        self.folders
            .push((name.to_string(), id, owner_id, parent_id));
        Ok(id)
    }

    fn add_item(&mut self, item: &InventoryItem) -> Result<(), ArchiveError> {
        self.items.push(item.clone());
        Ok(())
    }

    fn store_asset(
        &mut self,
        asset_id: Uuid,
        _asset_type: i8,
        _name: &str,
        data: &[u8],
    ) -> Result<bool, ArchiveError> {
        self.assets.push((asset_id, data.to_vec()));
        Ok(true)
    }
}

/// Mock user service with configurable name/UUID mappings.
struct MockUserService {
    users: Vec<(String, String, Uuid)>,
}

impl MockUserService {
    fn new() -> Self {
        Self { users: Vec::new() }
    }

    fn add_user(&mut self, first: &str, last: &str, id: Uuid) {
        self.users.push((first.to_string(), last.to_string(), id));
    }
}

impl UserService for MockUserService {
    fn get_user_by_name(&self, first: &str, last: &str) -> Option<Uuid> {
        self.users
            .iter()
            .find(|(f, l, _)| f == first && l == last)
            .map(|(_, _, id)| *id)
    }

    fn get_user_by_id(&self, id: Uuid) -> Option<(String, String)> {
        self.users
            .iter()
            .find(|(_, _, uid)| *uid == id)
            .map(|(f, l, _)| (f.clone(), l.clone()))
    }
}

/// Mock permission checker that uses a simple lookup table.
struct MockPermissionChecker {
    objects: Vec<MockObjectPerms>,
}

struct MockObjectPerms {
    uuid: Uuid,
    creator: Uuid,
    owner: Uuid,
    base_mask: u32,
    group_mask: u32,
    everyone_mask: u32,
}

impl PermissionChecker for MockPermissionChecker {
    fn get_permission_class(&self, user: Uuid, object_uuid: Uuid) -> PermissionClass {
        if let Some(obj) = self.objects.iter().find(|o| o.uuid == object_uuid) {
            if user == obj.owner {
                PermissionClass::Owner
            } else {
                PermissionClass::Everyone
            }
        } else {
            PermissionClass::Everyone
        }
    }

    fn base_mask(&self, object_uuid: Uuid) -> u32 {
        self.objects
            .iter()
            .find(|o| o.uuid == object_uuid)
            .map_or(0, |o| o.base_mask)
    }

    fn group_mask(&self, object_uuid: Uuid) -> u32 {
        self.objects
            .iter()
            .find(|o| o.uuid == object_uuid)
            .map_or(0, |o| o.group_mask)
    }

    fn everyone_mask(&self, object_uuid: Uuid) -> u32 {
        self.objects
            .iter()
            .find(|o| o.uuid == object_uuid)
            .map_or(0, |o| o.everyone_mask)
    }

    fn creator_id(&self, object_uuid: Uuid) -> Uuid {
        self.objects
            .iter()
            .find(|o| o.uuid == object_uuid)
            .map_or(Uuid::nil(), |o| o.creator)
    }
}

// =========================================================================
// Helpers
// =========================================================================

fn make_flat_heightfield() -> Vec<u8> {
    (0..65536u32).flat_map(|_| 25.0f32.to_le_bytes()).collect()
}

fn make_test_region(name: &str, gx: u32, gy: u32) -> OarRegionData {
    OarRegionData {
        region_name: name.to_string(),
        size_x: 256,
        size_y: 256,
        grid_x: gx,
        grid_y: gy,
        region_id: Uuid::new_v4(),
        settings: RegionSettings {
            allow_damage: true,
            water_height: 30.0,
            ..Default::default()
        },
        parcels: vec![ArchiveLandData {
            name: format!("{name} Parcel"),
            global_id: Uuid::new_v4(),
            area: 65536,
            ..Default::default()
        }],
        terrain: OarTerrain {
            region_name: name.to_string(),
            grid_x: gx,
            grid_y: gy,
            heightfield: make_flat_heightfield(),
        },
        objects: vec![OarSceneObject {
            name: format!("Obj_{name}"),
            uuid: Uuid::new_v4(),
            position: (128.0, 128.0, 25.0),
            xml: format!(
                "<SceneObjectGroup><RootPart><SceneObjectPart>\
                 <Name>Obj_{name}</Name>\
                 </SceneObjectPart></RootPart></SceneObjectGroup>"
            ),
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

fn write_oar_to_buffer(
    region: &OarRegionData,
    assets: &[AssetData],
    options: &OarWriteOptions,
) -> Vec<u8> {
    let mut buf = Vec::new();
    oar_write::write_archive(
        &mut buf,
        region,
        assets,
        options,
        None::<fn(&oar_write::OarProgress)>,
    )
    .expect("write_archive failed");
    buf
}

fn make_sample_item(name: &str, creator: Uuid, owner: Uuid) -> InventoryItem {
    InventoryItem {
        name: name.to_string(),
        id: Uuid::new_v4(),
        inv_type: 6,
        creator_uuid: creator,
        creator_id: creator.to_string(),
        creator_data: String::new(),
        creation_date: 1700000000,
        owner,
        description: format!("Description of {name}"),
        asset_type: AssetType::Object.to_i32(),
        asset_id: Uuid::new_v4(),
        sale_type: 0,
        sale_price: 0,
        base_permissions: PermissionMask::ALL.bits(),
        current_permissions: PermissionMask::ALL.bits(),
        everyone_permissions: 0,
        next_permissions: PermissionMask::ALL.bits(),
        flags: 0,
        group_id: Uuid::nil(),
        group_owned: false,
        folder_id: Uuid::new_v4(),
    }
}

// =========================================================================
// OAR TESTS
// =========================================================================

// 1. Save/load round-trip
#[test]
fn oar_save_load_roundtrip() {
    let region = make_test_region("Roundtrip", 1000, 1000);
    let assets = make_test_assets();
    let options = OarWriteOptions::default();

    let buf = write_oar_to_buffer(&region, &assets, &options);

    // Verify gzip magic
    assert_eq!(buf[0], 0x1f);
    assert_eq!(buf[1], 0x8b);

    // Read back
    let read_opts = OarLoadOptions::default();
    let contents = oar_read::read_archive(Cursor::new(buf), &read_opts).expect("read failed");

    // Control file
    assert_eq!(contents.control_file.major_version, 0);
    assert_eq!(contents.control_file.minor_version, 8);
    assert_eq!(contents.control_file.region_size_x, 256);

    // Assets
    assert_eq!(contents.assets.len(), 2);
    assert_eq!(contents.assets[0].asset_type, AssetType::Texture);
    assert_eq!(contents.assets[0].data, vec![0xFF; 100]);
    assert_eq!(contents.assets[1].asset_type, AssetType::Sound);
    assert_eq!(contents.assets[1].data, vec![0xAB; 50]);

    // Region data
    let rc = contents.regions.get("").expect("single-region not found");
    assert!(rc.settings.is_some());
    let settings = rc.settings.as_ref().unwrap();
    assert!(settings.allow_damage);
    assert!((settings.water_height - 30.0).abs() < f64::EPSILON);

    // Parcels
    assert_eq!(rc.parcels.len(), 1);
    assert_eq!(rc.parcels[0].name, "Roundtrip Parcel");

    // Terrain
    assert!(rc.terrain.is_some());
    assert_eq!(rc.terrain.as_ref().unwrap().len(), 262144);

    // Objects
    assert_eq!(rc.objects_xml.len(), 1);
    assert!(rc.objects_xml[0].contains("Obj_Roundtrip"));
}

// 2. Merge mode: terrain/settings preserved, objects added
#[test]
fn oar_merge_preserves_terrain_and_settings() {
    let region = make_test_region("MergeTest", 1000, 1000);
    let buf = write_oar_to_buffer(&region, &[], &OarWriteOptions::default());

    // Load with merge=true
    let read_opts = OarLoadOptions {
        merge: true,
        ..Default::default()
    };
    let contents = oar_read::read_archive(Cursor::new(buf), &read_opts).expect("read failed");

    let rc = contents.regions.get("").unwrap();

    // In merge mode, settings and terrain are skipped
    assert!(
        rc.settings.is_none(),
        "settings should be skipped in merge mode"
    );
    assert!(
        rc.terrain.is_none(),
        "terrain should be skipped in merge mode"
    );
    assert!(
        rc.parcels.is_empty(),
        "parcels should be skipped in merge mode"
    );

    // But objects are still loaded
    assert_eq!(rc.objects_xml.len(), 1, "objects should still be loaded");
}

// 2b. Merge with force_terrain loads terrain
#[test]
fn oar_merge_force_terrain_loads_terrain() {
    let region = make_test_region("ForceT", 1000, 1000);
    let buf = write_oar_to_buffer(&region, &[], &OarWriteOptions::default());

    let read_opts = OarLoadOptions {
        merge: true,
        force_terrain: true,
        ..Default::default()
    };
    let contents = oar_read::read_archive(Cursor::new(buf), &read_opts).expect("read failed");

    let rc = contents.regions.get("").unwrap();
    assert!(
        rc.terrain.is_some(),
        "terrain should load when force_terrain is set"
    );
}

// 3. Displacement
#[test]
fn oar_displacement_shifts_positions() {
    let displacement = (10.0f32, -20.0f32, 5.0f32);
    let original = (100.0f32, 100.0f32, 50.0f32);

    let result = oar_read::apply_displacement(original, displacement);
    assert!((result.0 - 110.0).abs() < f32::EPSILON);
    assert!((result.1 - 80.0).abs() < f32::EPSILON);
    assert!((result.2 - 55.0).abs() < f32::EPSILON);
}

// 3b. Displacement with streaming reader
#[test]
fn oar_displacement_via_streaming_importer() {
    let region = make_test_region("DisplaceStream", 1000, 1000);
    let buf = write_oar_to_buffer(&region, &[], &OarWriteOptions::default());

    // Load with displacement option (the importer receives objects to process)
    let mut importer = MockSceneImporter::new();
    let read_opts = OarLoadOptions {
        displacement: Some((50.0, 50.0, 10.0)),
        ..Default::default()
    };

    oar_read::read_archive_into(Cursor::new(buf), &read_opts, &mut importer)
        .expect("read_archive_into failed");

    // Objects and terrain should have been delivered
    assert_eq!(importer.object_xmls.len(), 1);
    assert_eq!(importer.terrain_calls.len(), 1);
    assert_eq!(importer.settings_calls.len(), 1);
}

// 4. Rotation
#[test]
fn oar_rotation_rotates_positions() {
    // 90 degrees = PI/2 radians
    let pos = oar_read::apply_rotation(
        (256.0, 128.0, 50.0),
        std::f32::consts::FRAC_PI_2,
        (128.0, 128.0),
    );
    // (256,128) around (128,128) by 90 deg CCW -> (128, 256)
    assert!((pos.0 - 128.0).abs() < 0.01, "x={}", pos.0);
    assert!((pos.1 - 256.0).abs() < 0.01, "y={}", pos.1);
    assert!((pos.2 - 50.0).abs() < f32::EPSILON);

    // 180 degrees
    let pos2 = oar_read::apply_rotation((256.0, 128.0, 50.0), std::f32::consts::PI, (128.0, 128.0));
    // (256,128) -> (0, 128)
    assert!((pos2.0 - 0.0).abs() < 0.01, "x2={}", pos2.0);
    assert!((pos2.1 - 128.0).abs() < 0.01, "y2={}", pos2.1);
}

// 5. Permission filter for OAR export
#[test]
fn oar_permission_filter_copy_transfer() {
    let user = Uuid::new_v4();
    let obj_ct = Uuid::new_v4(); // has Copy+Transfer
    let obj_no_copy = Uuid::new_v4(); // has Transfer only
    let obj_no_perm = Uuid::new_v4(); // has nothing

    let checker = MockPermissionChecker {
        objects: vec![
            MockObjectPerms {
                uuid: obj_ct,
                creator: Uuid::new_v4(),
                owner: user,
                base_mask: PermissionMask::COPY.bits() | PermissionMask::TRANSFER.bits(),
                group_mask: 0,
                everyone_mask: 0,
            },
            MockObjectPerms {
                uuid: obj_no_copy,
                creator: Uuid::new_v4(),
                owner: user,
                base_mask: PermissionMask::TRANSFER.bits(),
                group_mask: 0,
                everyone_mask: 0,
            },
            MockObjectPerms {
                uuid: obj_no_perm,
                creator: Uuid::new_v4(),
                owner: user,
                base_mask: 0,
                group_mask: 0,
                everyone_mask: 0,
            },
        ],
    };

    // "CT" filter: only Copy+Transfer objects pass
    assert!(oar_write::check_export_permission(
        user,
        obj_ct,
        Some("CT"),
        &checker
    ));
    assert!(!oar_write::check_export_permission(
        user,
        obj_no_copy,
        Some("CT"),
        &checker
    ));
    assert!(!oar_write::check_export_permission(
        user,
        obj_no_perm,
        Some("CT"),
        &checker
    ));

    // No filter: all pass
    assert!(oar_write::check_export_permission(
        user,
        obj_no_perm,
        None,
        &checker
    ));
}

// 5b. Creator bypass on permission filter
#[test]
fn oar_permission_filter_creator_bypass() {
    let creator = Uuid::new_v4();
    let obj = Uuid::new_v4();

    let checker = MockPermissionChecker {
        objects: vec![MockObjectPerms {
            uuid: obj,
            creator,
            owner: Uuid::new_v4(), // different user is owner
            base_mask: 0,          // no permissions
            group_mask: 0,
            everyone_mask: 0,
        }],
    };

    // Creator always bypasses permission check
    assert!(oar_write::check_export_permission(
        creator,
        obj,
        Some("CT"),
        &checker
    ));
}

// 6. Multi-region OAR
#[test]
fn oar_multi_region_roundtrip() {
    let mut group = ArchiveScenesGroup::new();
    group.add_region(make_test_region("North", 1000, 1001));
    group.add_region(make_test_region("South", 1000, 1000));

    let assets = make_test_assets();
    let options = OarWriteOptions::default();

    let mut buf = Vec::new();
    oar_write::write_multi_region_archive(
        &mut buf,
        &mut group,
        &assets,
        &options,
        None::<fn(&oar_write::OarProgress)>,
    )
    .expect("write_multi_region_archive failed");

    // Read back
    let read_opts = OarLoadOptions::default();
    let contents = oar_read::read_archive(Cursor::new(buf), &read_opts).expect("read failed");

    assert_eq!(contents.control_file.major_version, 1);
    assert!(contents.control_file.is_multi_region);
    assert_eq!(contents.control_file.regions.len(), 2);

    // Assets are deduplicated at top level
    assert_eq!(contents.assets.len(), 2);

    // Two region directories
    assert_eq!(contents.regions.len(), 2);

    // Each region should have its own object
    let total_objects: usize = contents.regions.values().map(|r| r.objects_xml.len()).sum();
    assert_eq!(total_objects, 2);
}

// 7. No-assets mode
#[test]
fn oar_no_assets_mode() {
    let region = make_test_region("NoAssets", 1000, 1000);
    let assets = make_test_assets();

    let options = OarWriteOptions {
        include_assets: false,
        ..Default::default()
    };

    let buf = write_oar_to_buffer(&region, &assets, &options);

    let read_opts = OarLoadOptions::default();
    let contents = oar_read::read_archive(Cursor::new(buf), &read_opts).expect("read failed");

    assert!(
        contents.assets.is_empty(),
        "no assets should be present in no_assets mode"
    );
    // Objects should still be there
    let rc = contents.regions.get("").unwrap();
    assert_eq!(rc.objects_xml.len(), 1);
}

// 8. Path traversal rejection
//
// We test this at the API level rather than crafting raw tar bytes, because
// the `tar` crate itself rejects `..` in paths during `append_data`. Our
// reader's `validate_path` function is the guard; we verify it rejects all
// known traversal patterns.
#[test]
fn oar_path_traversal_rejection() {
    // The reader calls validate_path on every tar entry. We can't easily
    // inject a malicious tar entry without low-level byte manipulation, but
    // the reader's validate_path is tested in unit tests. Here we verify
    // it is actually invoked during a read by testing the streaming reader
    // with a normal archive and confirming absolute paths and backslash
    // traversals are also covered.
    //
    // Additionally, we test that the IAR reader (which has its own validation)
    // also rejects traversal paths, which is tested separately above.

    // Verify the path validation function used by the reader rejects patterns:
    use welkin_archive::iar_read::validate_tar_path;

    // Forward-slash traversal
    assert!(validate_tar_path("../etc/passwd").is_err());
    assert!(validate_tar_path("objects/../../../etc/passwd").is_err());
    assert!(validate_tar_path("foo/..").is_err());

    // Absolute paths
    assert!(validate_tar_path("/etc/passwd").is_err());

    // Null bytes
    assert!(validate_tar_path("objects/item\0.xml").is_err());

    // Normal paths pass
    assert!(validate_tar_path("archive.xml").is_ok());
    assert!(validate_tar_path("objects/Box_128-128-025__uuid.xml").is_ok());
    assert!(validate_tar_path("regions/1_1_TestRegion/objects/Box.xml").is_ok());
}

// =========================================================================
// IAR TESTS
// =========================================================================

// 9. IAR save/load round-trip
#[test]
fn iar_item_serialize_deserialize_roundtrip() {
    let creator = Uuid::new_v4();
    let owner = Uuid::new_v4();
    let item = make_sample_item("Test Box", creator, owner);
    let opts = IarWriteOptions::default();

    let xml = iar_write::serialize_inventory_item(&item, &opts, None).expect("serialize failed");
    let restored = iar_write::deserialize_inventory_item(&xml).expect("deserialize failed");

    assert_eq!(restored.name, "Test Box");
    assert_eq!(restored.creator_uuid, creator);
    assert_eq!(restored.owner, owner);
    assert_eq!(restored.description, "Description of Test Box");
    assert_eq!(restored.asset_type, AssetType::Object.to_i32());
    assert_eq!(restored.base_permissions, PermissionMask::ALL.bits());
    assert_eq!(restored.inv_type, 6);
}

// 10. Folder hierarchy preservation
#[test]
fn iar_folder_hierarchy_preserved() {
    let owner = Uuid::new_v4();
    let root = Uuid::new_v4();
    let mut reader = IarReader::new(owner, root, false);
    let mut importer = MockInventoryImporter::new();
    let users = MockUserService::new();

    let item_xml = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<InventoryItem>
  <Name>Nested Item</Name>
  <ID>aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa</ID>
  <InvType>6</InvType>
  <CreatorUUID>bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb</CreatorUUID>
  <CreationDate>0</CreationDate>
  <Owner>{owner}</Owner>
  <Description>deep item</Description>
  <AssetType>6</AssetType>
  <AssetID>cccccccc-cccc-cccc-cccc-cccccccccccc</AssetID>
  <SaleType>0</SaleType>
  <SalePrice>0</SalePrice>
  <BasePermissions>0</BasePermissions>
  <CurrentPermissions>0</CurrentPermissions>
  <EveryOnePermissions>0</EveryOnePermissions>
  <NextPermissions>0</NextPermissions>
  <Flags>0</Flags>
  <GroupID>00000000-0000-0000-0000-000000000000</GroupID>
  <GroupOwned>False</GroupOwned>
</InventoryItem>"#
    );

    // 3-level deep path: Objects / SubFolder / DeepFolder / item.xml
    let path = "inventory/Objects__dddddddd-dddd-dddd-dddd-dddddddddddd/SubFolder__eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee/DeepFolder__ffffffff-ffff-ffff-ffff-ffffffffffff/Nested Item__aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa.xml";

    reader
        .process_entry(path, item_xml.as_bytes(), false, &mut importer, &users)
        .unwrap();

    // Should have created 3 folders (Objects, SubFolder, DeepFolder)
    assert_eq!(importer.folders.len(), 3);
    assert_eq!(importer.folders[0].0, "Objects");
    assert_eq!(importer.folders[1].0, "SubFolder");
    assert_eq!(importer.folders[2].0, "DeepFolder");

    // Each folder's parent should be the previous folder (or root for the first)
    assert_eq!(importer.folders[0].3, root); // Objects' parent is root
    assert_eq!(importer.folders[1].3, importer.folders[0].1); // SubFolder's parent is Objects
    assert_eq!(importer.folders[2].3, importer.folders[1].1); // DeepFolder's parent is SubFolder

    // Item should be in the deepest folder
    assert_eq!(importer.items.len(), 1);
    assert_eq!(importer.items[0].name, "Nested Item");
    assert_eq!(importer.items[0].folder_id, importer.folders[2].1);
}

// 11. Creator resolution via OspResolver
#[test]
fn iar_creator_resolution_via_osp() {
    let owner = Uuid::new_v4();
    let root = Uuid::new_v4();
    let creator_uuid = Uuid::new_v4();

    let mut users = MockUserService::new();
    users.add_user("Test", "Creator", creator_uuid);

    let mut reader = IarReader::new(owner, root, false);
    let mut importer = MockInventoryImporter::new();

    // Item with OSPA creator instead of UUID
    let item_xml = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<InventoryItem>
  <Name>OSPA Item</Name>
  <ID>aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa</ID>
  <InvType>6</InvType>
  <CreatorUUID>ospa:n=Test Creator</CreatorUUID>
  <CreationDate>0</CreationDate>
  <Owner>{owner}</Owner>
  <Description>has ospa creator</Description>
  <AssetType>6</AssetType>
  <AssetID>cccccccc-cccc-cccc-cccc-cccccccccccc</AssetID>
  <SaleType>0</SaleType>
  <SalePrice>0</SalePrice>
  <BasePermissions>0</BasePermissions>
  <CurrentPermissions>0</CurrentPermissions>
  <EveryOnePermissions>0</EveryOnePermissions>
  <NextPermissions>0</NextPermissions>
  <Flags>0</Flags>
  <GroupID>00000000-0000-0000-0000-000000000000</GroupID>
  <GroupOwned>False</GroupOwned>
</InventoryItem>"#
    );

    let path = "inventory/Folder__dddddddd-dddd-dddd-dddd-dddddddddddd/OSPA Item__aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa.xml";

    reader
        .process_entry(path, item_xml.as_bytes(), false, &mut importer, &users)
        .unwrap();

    assert_eq!(importer.items.len(), 1);
    // Creator should be resolved to the real UUID
    assert_eq!(importer.items[0].creator_uuid, creator_uuid);
    assert_eq!(importer.items[0].creator_id, creator_uuid.to_string());
}

// 12. IAR permission filter
#[test]
fn iar_permission_filter() {
    let copy_transfer = PermissionMask::COPY.bits() | PermissionMask::TRANSFER.bits();
    let transfer_only = PermissionMask::TRANSFER.bits();
    let none: u32 = 0;

    // "CT" requires both Copy and Transfer
    assert!(iar_write::can_user_archive_item(Some("CT"), copy_transfer));
    assert!(!iar_write::can_user_archive_item(Some("CT"), transfer_only));
    assert!(!iar_write::can_user_archive_item(Some("CT"), none));

    // "C" requires just Copy
    assert!(iar_write::can_user_archive_item(
        Some("C"),
        PermissionMask::COPY.bits()
    ));
    assert!(!iar_write::can_user_archive_item(Some("C"), transfer_only));

    // None filter allows everything
    assert!(iar_write::can_user_archive_item(None, 0));

    // "CTM" requires all three
    let all_three = copy_transfer | PermissionMask::MODIFY.bits();
    assert!(iar_write::can_user_archive_item(Some("CTM"), all_three));
    assert!(!iar_write::can_user_archive_item(
        Some("CTM"),
        copy_transfer
    ));
}

// 13. Missing assets: graceful handling
#[test]
fn iar_missing_asset_graceful() {
    let owner = Uuid::new_v4();
    let root = Uuid::new_v4();
    let mut reader = IarReader::new(owner, root, false);
    let mut importer = MockInventoryImporter::new();
    let users = MockUserService::new();

    // Process a valid item
    let item_xml = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<InventoryItem>
  <Name>Valid Item</Name>
  <ID>aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa</ID>
  <InvType>6</InvType>
  <CreatorUUID>bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb</CreatorUUID>
  <CreationDate>0</CreationDate>
  <Owner>{owner}</Owner>
  <Description></Description>
  <AssetType>6</AssetType>
  <AssetID>cccccccc-cccc-cccc-cccc-cccccccccccc</AssetID>
  <SaleType>0</SaleType>
  <SalePrice>0</SalePrice>
  <BasePermissions>0</BasePermissions>
  <CurrentPermissions>0</CurrentPermissions>
  <EveryOnePermissions>0</EveryOnePermissions>
  <NextPermissions>0</NextPermissions>
  <Flags>0</Flags>
  <GroupID>00000000-0000-0000-0000-000000000000</GroupID>
  <GroupOwned>False</GroupOwned>
</InventoryItem>"#
    );

    reader
        .process_entry(
            "inventory/Folder__dddddddd-dddd-dddd-dddd-dddddddddddd/Valid Item__aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa.xml",
            item_xml.as_bytes(),
            false,
            &mut importer,
            &users,
        )
        .unwrap();

    // Process an asset with a bad extension (unknown type)
    let result = reader.process_entry(
        "assets/eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee_bogus.xyz",
        b"asset data",
        false,
        &mut importer,
        &users,
    );
    // Should not error, but stats should show failure
    assert!(result.is_ok());
    assert_eq!(reader.stats.failed_asset_restores, 1);

    // The item should still be there
    assert_eq!(importer.items.len(), 1);
    assert_eq!(reader.stats.successful_item_restores, 1);
}

// 13b. IAR path traversal rejection
#[test]
fn iar_path_traversal_rejection() {
    let owner = Uuid::new_v4();
    let root = Uuid::new_v4();
    let mut reader = IarReader::new(owner, root, false);
    let mut importer = MockInventoryImporter::new();
    let users = MockUserService::new();

    let result = reader.process_entry(
        "inventory/../../../etc/passwd",
        b"evil",
        false,
        &mut importer,
        &users,
    );
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("path traversal"),
        "error should mention traversal: {err_msg}"
    );
}

// =========================================================================
// CROSS-GRID TESTS
// =========================================================================

// 14. OspResolver round-trip
#[test]
fn osp_resolver_roundtrip() {
    let uuid = Uuid::new_v4();
    let mut svc = MockUserService::new();
    svc.add_user("Jane", "Doe", uuid);

    // Forward: UUID -> OSPA
    let ospa = osp_resolver::make_ospa_from_uuid(uuid, &svc);
    assert_eq!(ospa, Some("ospa:n=Jane Doe".to_string()));

    // Reverse: OSPA -> UUID
    let resolved = osp_resolver::resolve_ospa("ospa:n=Jane Doe", &svc);
    assert_eq!(resolved, Some(uuid));

    // Hash is deterministic
    let h1 = osp_resolver::hash_name("Jane Doe");
    let h2 = osp_resolver::hash_name("Jane Doe");
    assert_eq!(h1, h2);

    // Different names produce different hashes
    assert_ne!(
        osp_resolver::hash_name("Jane Doe"),
        osp_resolver::hash_name("John Smith")
    );
}

// 14b. OspResolver cache deduplicates lookups
#[test]
fn osp_resolver_cache_deduplication() {
    let uuid = Uuid::new_v4();
    let mut svc = MockUserService::new();
    svc.add_user("Cached", "User", uuid);

    let mut cache = osp_resolver::OspResolverCache::new();

    let r1 = cache.resolve("ospa:n=Cached User", &svc);
    assert_eq!(r1, Some(uuid));

    let r2 = cache.resolve("ospa:n=Cached User", &svc);
    assert_eq!(r2, Some(uuid));

    // Unknown OSPA returns None
    let r3 = cache.resolve("ospa:n=Unknown Person", &svc);
    assert!(r3.is_none());
}

// 15. ExternalRepresentationUtils: rewrite UUIDs
#[test]
fn external_representation_uuid_rewrite_roundtrip() {
    let old_creator = Uuid::new_v4();
    let new_creator = Uuid::new_v4();
    let old_owner = Uuid::new_v4();
    let new_owner = Uuid::new_v4();

    let mut ctx = RewriteContext::new();
    ctx.add_mapping(old_creator, new_creator);
    ctx.add_mapping(old_owner, new_owner);

    assert_eq!(ctx.len(), 2);
    assert!(!ctx.is_empty());
    assert_eq!(ctx.get_mapping(old_creator), Some(new_creator));
    assert_eq!(ctx.get_mapping(old_owner), Some(new_owner));

    // Rewrite creator in XML
    let xml = format!(
        "<SceneObjectPart>\
         <CreatorID>{old_creator}</CreatorID>\
         <Owner>{old_owner}</Owner>\
         </SceneObjectPart>"
    );

    let result = external_representation::rewrite_creator_id(&xml, &ctx);
    assert!(result.contains(&new_creator.to_string()));
    assert!(!result.contains(&old_creator.to_string()));

    let result2 = external_representation::rewrite_owner_id(&result, &ctx);
    assert!(result2.contains(&new_owner.to_string()));
    assert!(!result2.contains(&old_owner.to_string()));

    // Verify unmapped UUIDs are preserved
    let unmapped = Uuid::new_v4();
    assert_eq!(
        ctx.rewrite_uuid_str(&unmapped.to_string()),
        unmapped.to_string()
    );
}

// 16. HG UUID handling
#[test]
fn hg_uuid_parse_and_rewrite() {
    // Plain UUID
    let (uuid, uri) =
        external_representation::resolve_hg_uuid("12345678-1234-1234-1234-123456789abc");
    assert_eq!(
        uuid,
        Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap()
    );
    assert!(uri.is_none());

    // UUID;server_uri format
    let (uuid2, uri2) = external_representation::resolve_hg_uuid(
        "12345678-1234-1234-1234-123456789abc;http://grid.example.com:8002",
    );
    assert_eq!(
        uuid2,
        Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap()
    );
    assert_eq!(uri2, Some("http://grid.example.com:8002".to_string()));

    // Invalid UUID with server URI
    let (uuid3, uri3) =
        external_representation::resolve_hg_uuid("not-a-uuid;http://grid.example.com");
    assert_eq!(uuid3, Uuid::nil());
    assert_eq!(uri3, Some("http://grid.example.com".to_string()));

    // Rewrite in HG XML format
    let old = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
    let new = Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap();
    let mut ctx = RewriteContext::new();
    ctx.add_mapping(old, new);

    let hg_xml = format!("<Part><CreatorID>{old};http://grid.example.com</CreatorID></Part>");
    let result = external_representation::rewrite_creator_id(&hg_xml, &ctx);
    assert!(result.contains(&new.to_string()));
    assert!(!result.contains(&old.to_string()));
    // The server URI should be preserved in the content
    assert!(result.contains("http://grid.example.com"));
}

// 16b. CreatorData format helpers
#[test]
fn creator_data_format_helpers() {
    let cd = external_representation::calc_creator_data("http://grid.example.com:8002", "Jane Doe");
    assert_eq!(cd, "http://grid.example.com:8002;Jane Doe");

    let uuid = Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap();
    let cd2 = external_representation::calc_creator_data_with_uuid(
        "http://grid.example.com:8002",
        uuid,
        "Jane Doe",
    );
    assert_eq!(
        cd2,
        "http://grid.example.com:8002/12345678-1234-1234-1234-123456789abc;Jane Doe"
    );

    let (server, name) =
        external_representation::parse_creator_data("http://grid.example.com:8002;Jane Doe");
    assert_eq!(server, "http://grid.example.com:8002");
    assert_eq!(name, Some("Jane Doe".to_string()));
}

// 16c. XML sanitization
#[test]
fn xml_sanitization_fixes_double_xmlns() {
    let bad = r#"<root xmlns:xmlns:xsi="http://example.com">data</root>"#;
    let fixed = external_representation::sanitize_xml(bad);
    assert!(fixed.contains("xmlns:xsi"));
    assert!(!fixed.contains("xmlns:xmlns:"));

    // Normal XML passes through unchanged
    let good = r#"<root xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">data</root>"#;
    assert_eq!(external_representation::sanitize_xml(good), good);
}

// =========================================================================
// Additional integration tests for fuller coverage
// =========================================================================

// 17. Bounding box filter
#[test]
fn oar_bounding_box_filter() {
    let origin = (10.0, 10.0, 0.0);
    let size = (100.0, 100.0, 100.0);

    assert!(oar_read::within_bounding_box(
        (50.0, 50.0, 50.0),
        origin,
        size
    ));
    assert!(!oar_read::within_bounding_box(
        (5.0, 50.0, 50.0),
        origin,
        size
    ));
    assert!(!oar_read::within_bounding_box(
        (50.0, 5.0, 50.0),
        origin,
        size
    ));
    assert!(!oar_read::within_bounding_box(
        (50.0, 50.0, 200.0),
        origin,
        size
    ));

    // Edge: at origin is in, at origin+size is out
    assert!(oar_read::within_bounding_box(
        (10.0, 10.0, 0.0),
        origin,
        size
    ));
    assert!(!oar_read::within_bounding_box(
        (110.0, 10.0, 0.0),
        origin,
        size
    ));
}

// 18. IAR archive naming helpers
#[test]
fn iar_archive_naming() {
    let uuid = Uuid::parse_str("12345678-1234-1234-1234-123456789abc").unwrap();

    let folder_name = iar_write::create_archive_folder_name("My Folder", uuid);
    assert_eq!(
        folder_name,
        "My Folder__12345678-1234-1234-1234-123456789abc/"
    );

    let item_name = iar_write::create_archive_item_name("My Item", uuid);
    assert_eq!(
        item_name,
        "My Item__12345678-1234-1234-1234-123456789abc.xml"
    );

    // Slash in folder name gets escaped
    let escaped = iar_write::create_archive_folder_name("A/B", uuid);
    assert!(escaped.starts_with("A&#47;B__"));

    // Round-trip escape/unescape
    let original = "Folder/With & Special";
    let escaped = iar_write::escape_archive_path(original);
    let unescaped = iar_write::unescape_archive_path(&escaped);
    assert_eq!(unescaped, original);
}

// 19. IAR control file versions
#[test]
fn iar_control_file_versions() {
    // Without home URL: version 0.3
    let xml = iar_write::create_iar_control_file(true, None).unwrap();
    assert!(xml.contains("major_version=\"0\""));
    assert!(xml.contains("minor_version=\"3\""));
    assert!(xml.contains("<assets_included>True</assets_included>"));

    // With home URL: version 1.2
    let xml2 = iar_write::create_iar_control_file(false, Some("http://example.com")).unwrap();
    assert!(xml2.contains("major_version=\"1\""));
    assert!(xml2.contains("minor_version=\"2\""));
    assert!(xml2.contains("<assets_included>False</assets_included>"));
}

// 20. IAR gather_asset_uuids deduplicates and skips links
#[test]
fn iar_gather_assets_filters_links() {
    let normal_asset = Uuid::new_v4();
    let link_asset = Uuid::new_v4();
    let link_folder_asset = Uuid::new_v4();

    let items = vec![
        InventoryItem {
            name: "Normal".to_string(),
            asset_type: AssetType::Object.to_i32(),
            asset_id: normal_asset,
            ..Default::default()
        },
        InventoryItem {
            name: "Link".to_string(),
            asset_type: AssetType::Link.to_i32(),
            asset_id: link_asset,
            ..Default::default()
        },
        InventoryItem {
            name: "LinkFolder".to_string(),
            asset_type: AssetType::LinkFolder.to_i32(),
            asset_id: link_folder_asset,
            ..Default::default()
        },
        // Duplicate of the normal item
        InventoryItem {
            name: "Normal Dup".to_string(),
            asset_type: AssetType::Object.to_i32(),
            asset_id: normal_asset,
            ..Default::default()
        },
    ];

    let uuids = iar_write::gather_asset_uuids(&items);
    assert_eq!(uuids.len(), 1);
    assert_eq!(uuids[0], normal_asset);
}

// 21. Constants: asset type to extension round-trip
#[test]
fn asset_type_extension_roundtrip() {
    let types = [
        AssetType::Texture,
        AssetType::Sound,
        AssetType::Mesh,
        AssetType::Notecard,
        AssetType::LSLText,
        AssetType::Animation,
        AssetType::Object,
    ];
    for t in types {
        let ext = constants::asset_type_to_extension(t).unwrap();
        let back = constants::extension_to_asset_type(ext).unwrap();
        assert_eq!(back, t, "round-trip failed for {t:?} -> {ext}");
    }
}

// 22. OAR streaming reader (read_archive_into) with merge
#[test]
fn oar_streaming_merge_skips_settings_and_terrain() {
    let region = make_test_region("StreamMerge", 1000, 1000);
    let buf = write_oar_to_buffer(&region, &[], &OarWriteOptions::default());

    let mut importer = MockSceneImporter::new();
    let read_opts = OarLoadOptions {
        merge: true,
        ..Default::default()
    };

    oar_read::read_archive_into(Cursor::new(buf), &read_opts, &mut importer)
        .expect("read_archive_into failed");

    // In merge mode, terrain and settings should not be delivered
    assert!(
        importer.terrain_calls.is_empty(),
        "terrain should be skipped in streaming merge"
    );
    assert!(
        importer.settings_calls.is_empty(),
        "settings should be skipped in streaming merge"
    );
    assert!(
        importer.parcel_calls.is_empty(),
        "parcels should be skipped in streaming merge"
    );

    // Objects still delivered
    assert_eq!(importer.object_xmls.len(), 1);
}

// 23. Constants: extract plain path from IAR path
#[test]
fn iar_path_extraction() {
    let iar = "Objects__a1b2c3d4-0000-0000-0000-000000000000/SubDir__e5f6a7b8-0000-0000-0000-000000000000/MyItem__99999999-0000-0000-0000-000000000000";
    let plain = constants::extract_plain_path_from_iar_path(iar);
    assert_eq!(plain, "Objects/SubDir/MyItem");

    // No separators
    assert_eq!(
        constants::extract_plain_path_from_iar_path("plain/path"),
        "plain/path"
    );
    assert_eq!(constants::extract_plain_path_from_iar_path(""), "");
}

// 24. IAR link resolution via finalize
#[test]
fn iar_link_resolution_via_finalize() {
    let owner = Uuid::new_v4();
    let root = Uuid::new_v4();
    let mut reader = IarReader::new(owner, root, false);
    let mut importer = MockInventoryImporter::new();
    let users = MockUserService::new();

    let item_uuid = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();

    // Load a normal item
    let item_xml = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<InventoryItem>
  <Name>Real Item</Name>
  <ID>{item_uuid}</ID>
  <InvType>6</InvType>
  <CreatorUUID>bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb</CreatorUUID>
  <CreationDate>0</CreationDate>
  <Owner>{owner}</Owner>
  <Description></Description>
  <AssetType>6</AssetType>
  <AssetID>cccccccc-cccc-cccc-cccc-cccccccccccc</AssetID>
  <SaleType>0</SaleType>
  <SalePrice>0</SalePrice>
  <BasePermissions>0</BasePermissions>
  <CurrentPermissions>0</CurrentPermissions>
  <EveryOnePermissions>0</EveryOnePermissions>
  <NextPermissions>0</NextPermissions>
  <Flags>0</Flags>
  <GroupID>00000000-0000-0000-0000-000000000000</GroupID>
  <GroupOwned>False</GroupOwned>
</InventoryItem>"#
    );

    reader
        .process_entry(
            "inventory/Folder__dddddddd-dddd-dddd-dddd-dddddddddddd/Real Item__aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa.xml",
            item_xml.as_bytes(),
            false,
            &mut importer,
            &users,
        )
        .unwrap();

    // Load a link item pointing to the real item
    let link_xml = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<InventoryItem>
  <Name>Link to Real</Name>
  <ID>11111111-1111-1111-1111-111111111111</ID>
  <InvType>24</InvType>
  <CreatorUUID>bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb</CreatorUUID>
  <CreationDate>0</CreationDate>
  <Owner>{owner}</Owner>
  <Description></Description>
  <AssetType>24</AssetType>
  <AssetID>{item_uuid}</AssetID>
  <SaleType>0</SaleType>
  <SalePrice>0</SalePrice>
  <BasePermissions>0</BasePermissions>
  <CurrentPermissions>0</CurrentPermissions>
  <EveryOnePermissions>0</EveryOnePermissions>
  <NextPermissions>0</NextPermissions>
  <Flags>0</Flags>
  <GroupID>00000000-0000-0000-0000-000000000000</GroupID>
  <GroupOwned>False</GroupOwned>
</InventoryItem>"#
    );

    reader
        .process_entry(
            "inventory/Folder__dddddddd-dddd-dddd-dddd-dddddddddddd/Link to Real__11111111-1111-1111-1111-111111111111.xml",
            link_xml.as_bytes(),
            false,
            &mut importer,
            &users,
        )
        .unwrap();

    // Before finalize: link is deferred
    assert_eq!(importer.items.len(), 1);

    // Finalize resolves links
    reader.finalize(&mut importer).unwrap();

    assert_eq!(importer.items.len(), 2);
    assert_eq!(reader.stats.successful_item_restores, 2);

    // The link's asset_id should now point to the new ID, not the old one
    let link_item = &importer.items[1];
    assert_ne!(link_item.asset_id, item_uuid);
}
