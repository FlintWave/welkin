// SPDX-License-Identifier: GPL-3.0-or-later

//! IAR (Inventory ARchive) read support.
//!
//! Provides the `IarReader` for reading IAR archives and the
//! `InventoryImporter` trait for plugging into an inventory backend.
//!
//! Ported from `InventoryArchiveReadRequest.cs`.

use std::collections::HashMap;
use tracing::{debug, error, warn};
use uuid::Uuid;

use crate::constants::{
    self, ASSETS_PATH, CONTROL_FILE_PATH, INVENTORY_NODE_NAME_COMPONENT_SEPARATOR,
    INVENTORY_PATH,
};
use crate::iar_write::{deserialize_inventory_item, unescape_archive_path, InventoryItem};
use crate::osp_resolver::{OspResolverCache, UserService};
use crate::ArchiveError;

use welkin_types::AssetType;

// ---------------------------------------------------------------------------
// Security constants
// ---------------------------------------------------------------------------

/// Maximum allowed size for a single asset in an IAR (256 MiB).
pub const MAX_ASSET_SIZE: usize = 256 * 1024 * 1024;

/// Maximum allowed size for an inventory item XML file (16 MiB).
pub const MAX_ITEM_XML_SIZE: usize = 16 * 1024 * 1024;

/// Maximum supported IAR major version.
pub const MAX_MAJOR_VERSION: i32 = 1;

// ---------------------------------------------------------------------------
// Inventory importer trait
// ---------------------------------------------------------------------------

/// Trait for plugging in actual inventory operations during an IAR load.
///
/// The IAR reader calls these methods to create folders, items, and store
/// assets in whatever backend is in use.
pub trait InventoryImporter {
    /// Create a folder in the user's inventory.
    ///
    /// Returns the UUID assigned to the new folder.
    fn create_folder(
        &mut self,
        name: &str,
        owner_id: Uuid,
        parent_id: Uuid,
    ) -> Result<Uuid, ArchiveError>;

    /// Add an inventory item.
    fn add_item(&mut self, item: &InventoryItem) -> Result<(), ArchiveError>;

    /// Store an asset blob. Returns `true` if stored successfully.
    fn store_asset(
        &mut self,
        asset_id: Uuid,
        asset_type: i8,
        name: &str,
        data: &[u8],
    ) -> Result<bool, ArchiveError>;
}

// ---------------------------------------------------------------------------
// Path-traversal validation
// ---------------------------------------------------------------------------

/// Validate a tar entry path for safety.
///
/// **Security**: rejects paths that:
/// - Contain `..` components (directory traversal)
/// - Start with `/` (absolute paths)
/// - Contain null bytes
pub fn validate_tar_path(path: &str) -> Result<(), ArchiveError> {
    if path.contains('\0') {
        return Err(ArchiveError::Invalid(
            "tar entry path contains null byte".to_string(),
        ));
    }

    if path.starts_with('/') {
        return Err(ArchiveError::Invalid(format!(
            "tar entry has absolute path: {path}"
        )));
    }

    for component in path.split('/') {
        if component == ".." {
            return Err(ArchiveError::Invalid(format!(
                "tar entry has path traversal: {path}"
            )));
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// IAR Reader
// ---------------------------------------------------------------------------

/// Statistics from an IAR load operation.
#[derive(Debug, Clone, Default)]
pub struct IarLoadStats {
    /// Number of inventory items successfully restored.
    pub successful_item_restores: u32,
    /// Number of assets successfully restored.
    pub successful_asset_restores: u32,
    /// Number of asset restore failures.
    pub failed_asset_restores: u32,
}

/// State machine for reading an IAR archive.
///
/// Feed it tar entries in order (control file first, then inventory, then
/// assets) using the `process_entry` method.
#[derive(Debug)]
pub struct IarReader {
    /// The user who owns this import.
    owner_id: Uuid,
    /// Root destination folder for the import.
    root_folder_id: Uuid,
    /// Whether to merge with existing inventory (reserved for future use).
    _merge: bool,
    /// Has the control file been processed?
    control_file_loaded: bool,
    /// Have any inventory entries been seen?
    inventory_nodes_loaded: bool,
    /// Have any asset entries been seen?
    assets_loaded: bool,
    /// Load statistics.
    pub stats: IarLoadStats,
    /// Resolved folder paths: iar_path → (folder_id, folder_name).
    resolved_folders: HashMap<String, (Uuid, String)>,
    /// Maps old item IDs to new item IDs (for link resolution).
    item_id_map: HashMap<Uuid, Uuid>,
    /// Deferred inventory links to resolve after all items are loaded.
    deferred_links: Vec<InventoryItem>,
    /// Creator ID to associate with each asset for later rewrites.
    creator_for_asset: HashMap<Uuid, Uuid>,
    /// OSP resolver cache.
    osp_cache: OspResolverCache,
}

impl IarReader {
    /// Create a new IAR reader.
    ///
    /// - `owner_id`: UUID of the user receiving the imported inventory.
    /// - `root_folder_id`: the destination folder in the user's inventory.
    /// - `merge`: if true, try to merge with existing folders by name.
    pub fn new(owner_id: Uuid, root_folder_id: Uuid, merge: bool) -> Self {
        Self {
            owner_id,
            root_folder_id,
            _merge: merge,
            control_file_loaded: true, // match C# behaviour for pre-0.2 IARs
            inventory_nodes_loaded: false,
            assets_loaded: false,
            stats: IarLoadStats::default(),
            resolved_folders: HashMap::new(),
            item_id_map: HashMap::new(),
            deferred_links: Vec::new(),
            creator_for_asset: HashMap::new(),
            osp_cache: OspResolverCache::new(),
        }
    }

    /// Process a single tar entry.
    ///
    /// Call this for each entry in the tar archive, in order.
    ///
    /// **Security**: validates the path and enforces size limits.
    pub fn process_entry(
        &mut self,
        path: &str,
        data: &[u8],
        is_directory: bool,
        importer: &mut dyn InventoryImporter,
        user_service: &dyn UserService,
    ) -> Result<(), ArchiveError> {
        validate_tar_path(path)?;

        if path == CONTROL_FILE_PATH {
            self.load_control_file(data)?;
        } else if let Some(suffix) = path.strip_prefix(ASSETS_PATH) {
            if !suffix.is_empty() {
                self.load_asset(path, data, importer)?;
            }
        } else if let Some(suffix) = path.strip_prefix(INVENTORY_PATH) {
            if !suffix.is_empty() {
                self.load_inventory_entry(
                    suffix,
                    data,
                    is_directory,
                    importer,
                    user_service,
                )?;
            }
        }
        // Silently ignore entries with unrecognised prefixes (e.g. userprofiles/)

        Ok(())
    }

    /// Finalize the load: resolve deferred inventory links.
    ///
    /// Call this after all tar entries have been processed.
    pub fn finalize(&mut self, importer: &mut dyn InventoryImporter) -> Result<(), ArchiveError> {
        for mut link in self.deferred_links.drain(..) {
            if let Some(&new_target) = self.item_id_map.get(&link.asset_id) {
                link.asset_id = new_target;
                if let Err(e) = importer.add_item(&link) {
                    warn!(name = %link.name, error = %e, "failed to add inventory link");
                } else {
                    self.stats.successful_item_restores += 1;
                }
            } else {
                warn!(
                    name = %link.name,
                    target = %link.asset_id,
                    "link target not found in loaded items, skipping"
                );
            }
        }

        self.item_id_map.clear();
        Ok(())
    }

    // -- Private helpers ---------------------------------------------------

    fn load_control_file(&mut self, data: &[u8]) -> Result<(), ArchiveError> {
        let text = std::str::from_utf8(data)
            .map_err(|e| ArchiveError::Invalid(format!("control file not UTF-8: {e}")))?;

        // Parse minimal control-file XML to check version
        let mut major: Option<i32> = None;

        // Simple attribute extraction without full XML parse
        if let Some(idx) = text.find("major_version") {
            if let Some(start) = text[idx..].find('"') {
                let after_quote = idx + start + 1;
                if let Some(end) = text[after_quote..].find('"') {
                    if let Ok(v) = text[after_quote..after_quote + end].parse::<i32>() {
                        major = Some(v);
                    }
                }
            }
        }

        if let Some(v) = major {
            if v > MAX_MAJOR_VERSION {
                return Err(ArchiveError::Invalid(format!(
                    "IAR major version {v} exceeds maximum supported version {MAX_MAJOR_VERSION}"
                )));
            }
        }

        self.control_file_loaded = true;
        debug!("IAR control file loaded");
        Ok(())
    }

    fn load_inventory_entry(
        &mut self,
        relative_path: &str,
        data: &[u8],
        is_directory: bool,
        importer: &mut dyn InventoryImporter,
        user_service: &dyn UserService,
    ) -> Result<(), ArchiveError> {
        if !self.control_file_loaded {
            return Err(ArchiveError::Invalid(
                "inventory entries found before control file".to_string(),
            ));
        }

        if self.assets_loaded {
            return Err(ArchiveError::Invalid(
                "inventory entries found after asset entries".to_string(),
            ));
        }

        // Determine the folder path (strip item filename if not a directory)
        let folder_path = if is_directory {
            relative_path.to_string()
        } else {
            match relative_path.rfind('/') {
                Some(idx) => relative_path[..idx + 1].to_string(),
                None => String::new(),
            }
        };

        // Ensure the folder hierarchy exists
        let folder_id = self.ensure_folders(&folder_path, importer)?;

        // If this is an item (not a directory entry), load it
        if !is_directory && !data.is_empty() {
            // Security: item XML size limit
            if data.len() > MAX_ITEM_XML_SIZE {
                return Err(ArchiveError::Invalid(format!(
                    "item XML size {} exceeds limit {}",
                    data.len(),
                    MAX_ITEM_XML_SIZE
                )));
            }

            let text = std::str::from_utf8(data)
                .map_err(|e| ArchiveError::Invalid(format!("item XML not UTF-8: {e}")))?;

            let mut item = deserialize_inventory_item(text)?;

            // Save old ID for link resolution
            let old_id = item.id;

            // Assign a new UUID (don't reuse the one from the archive)
            let new_id = Uuid::new_v4();
            item.id = new_id;
            self.item_id_map.insert(old_id, new_id);

            // Resolve OSPA creator to a real UUID
            if let Some(resolved) =
                self.osp_cache.resolve(&item.creator_id, user_service)
            {
                item.creator_id = resolved.to_string();
                item.creator_uuid = resolved;
                item.creator_data = String::new();
            } else if item.creator_data.is_empty() && item.creator_uuid.is_nil() {
                // Fall back to the importing user
                item.creator_id = self.owner_id.to_string();
                item.creator_uuid = self.owner_id;
            }

            // Set owner and folder
            item.owner = self.owner_id;
            item.folder_id = folder_id;

            // Defer link-type items for resolution after all items are loaded
            if item.asset_type == AssetType::Link.to_i32() {
                self.deferred_links.push(item);
            } else {
                self.creator_for_asset.insert(item.asset_id, item.creator_uuid);
                if let Err(e) = importer.add_item(&item) {
                    warn!(name = %item.name, error = %e, "failed to add item");
                } else {
                    self.stats.successful_item_restores += 1;
                }
            }
        }

        self.inventory_nodes_loaded = true;
        Ok(())
    }

    fn load_asset(
        &mut self,
        path: &str,
        data: &[u8],
        importer: &mut dyn InventoryImporter,
    ) -> Result<(), ArchiveError> {
        if !self.control_file_loaded {
            return Err(ArchiveError::Invalid(
                "asset entries found before control file".to_string(),
            ));
        }

        // Security: asset size limit
        if data.len() > MAX_ASSET_SIZE {
            warn!(
                path = path,
                size = data.len(),
                limit = MAX_ASSET_SIZE,
                "skipping oversized asset"
            );
            self.stats.failed_asset_restores += 1;
            return Ok(());
        }

        let filename = &path[ASSETS_PATH.len()..];

        // Find the extension separator
        let sep_idx = match filename.rfind(constants::ASSET_EXTENSION_SEPARATOR) {
            Some(idx) if idx >= 32 => idx,
            _ => {
                error!(path = path, "could not parse asset filename");
                self.stats.failed_asset_restores += 1;
                return Ok(());
            }
        };

        let raw_uuid = &filename[..sep_idx];
        let extension = &filename[sep_idx..];

        let asset_id = match Uuid::parse_str(raw_uuid) {
            Ok(id) => id,
            Err(_) => {
                error!(path = path, "invalid UUID in asset filename");
                self.stats.failed_asset_restores += 1;
                return Ok(());
            }
        };

        let asset_type = match constants::extension_to_asset_type(extension) {
            Some(t) => t.to_i32() as i8,
            None => {
                error!(path = path, ext = extension, "unknown asset extension");
                self.stats.failed_asset_restores += 1;
                return Ok(());
            }
        };

        match importer.store_asset(asset_id, asset_type, "From IAR", data) {
            Ok(true) => {
                self.stats.successful_asset_restores += 1;
            }
            Ok(false) => {
                self.stats.failed_asset_restores += 1;
            }
            Err(e) => {
                warn!(asset = %asset_id, error = %e, "failed to store asset");
                self.stats.failed_asset_restores += 1;
            }
        }

        if self.stats.successful_asset_restores % 50 == 0
            && self.stats.successful_asset_restores > 0
        {
            debug!(count = self.stats.successful_asset_restores, "loaded assets");
        }

        self.assets_loaded = true;
        Ok(())
    }

    /// Ensure all folders in the given IAR path exist, creating them as needed.
    ///
    /// Returns the UUID of the deepest folder.
    fn ensure_folders(
        &mut self,
        iar_path: &str,
        importer: &mut dyn InventoryImporter,
    ) -> Result<Uuid, ArchiveError> {
        if iar_path.is_empty() {
            return Ok(self.root_folder_id);
        }

        // Check if we've already resolved this exact path
        if let Some((folder_id, _)) = self.resolved_folders.get(iar_path) {
            return Ok(*folder_id);
        }

        // Build up the path incrementally
        let components: Vec<&str> = iar_path
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();

        let mut current_path = String::new();
        let mut parent_id = self.root_folder_id;

        for component in &components {
            current_path.push_str(component);
            current_path.push('/');

            if let Some((folder_id, _)) = self.resolved_folders.get(&current_path) {
                parent_id = *folder_id;
                continue;
            }

            // Extract the human-readable name from "Name__uuid" format
            let folder_name =
                if let Some(sep_idx) = component.rfind(INVENTORY_NODE_NAME_COMPONENT_SEPARATOR)
                {
                    let raw_name = &component[..sep_idx];
                    unescape_archive_path(raw_name)
                } else {
                    component.to_string()
                };

            // Create the folder
            let new_folder_id =
                importer.create_folder(&folder_name, self.owner_id, parent_id)?;

            self.resolved_folders
                .insert(current_path.clone(), (new_folder_id, folder_name));

            parent_id = new_folder_id;
        }

        Ok(parent_id)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::osp_resolver::UserService as OspUserService;

    /// Mock inventory importer for testing.
    struct MockImporter {
        folders: Vec<(String, Uuid, Uuid, Uuid)>, // (name, id, owner, parent)
        items: Vec<InventoryItem>,
        assets: Vec<(Uuid, Vec<u8>)>,
    }

    impl MockImporter {
        fn new() -> Self {
            Self {
                folders: Vec::new(),
                items: Vec::new(),
                assets: Vec::new(),
            }
        }
    }

    impl InventoryImporter for MockImporter {
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

    /// Mock user service.
    struct MockUsers;

    impl OspUserService for MockUsers {
        fn get_user_by_name(&self, _first: &str, _last: &str) -> Option<Uuid> {
            None
        }
        fn get_user_by_id(&self, _id: Uuid) -> Option<(String, String)> {
            None
        }
    }

    #[test]
    fn validate_tar_path_safe() {
        assert!(validate_tar_path("inventory/Folder__uuid/item__uuid.xml").is_ok());
        assert!(validate_tar_path("assets/uuid_texture.jp2").is_ok());
        assert!(validate_tar_path("archive.xml").is_ok());
    }

    #[test]
    fn validate_tar_path_traversal() {
        assert!(validate_tar_path("../etc/passwd").is_err());
        assert!(validate_tar_path("inventory/../../etc/passwd").is_err());
        assert!(validate_tar_path("inventory/Folder__uuid/../secret").is_err());
    }

    #[test]
    fn validate_tar_path_absolute() {
        assert!(validate_tar_path("/etc/passwd").is_err());
        assert!(validate_tar_path("/inventory/item.xml").is_err());
    }

    #[test]
    fn validate_tar_path_null_byte() {
        assert!(validate_tar_path("inventory/item\0.xml").is_err());
    }

    #[test]
    fn process_control_file() {
        let owner = Uuid::new_v4();
        let root = Uuid::new_v4();
        let mut reader = IarReader::new(owner, root, false);
        let mut importer = MockImporter::new();
        let users = MockUsers;

        let control_xml = br#"<?xml version="1.0"?>
<archive major_version="0" minor_version="3">
  <assets_included>True</assets_included>
</archive>"#;

        reader
            .process_entry("archive.xml", control_xml, false, &mut importer, &users)
            .unwrap();
        assert!(reader.control_file_loaded);
    }

    #[test]
    fn reject_future_version() {
        let owner = Uuid::new_v4();
        let root = Uuid::new_v4();
        let mut reader = IarReader::new(owner, root, false);
        let mut importer = MockImporter::new();
        let users = MockUsers;

        let control_xml = br#"<?xml version="1.0"?>
<archive major_version="99" minor_version="0">
  <assets_included>True</assets_included>
</archive>"#;

        let result =
            reader.process_entry("archive.xml", control_xml, false, &mut importer, &users);
        assert!(result.is_err());
    }

    #[test]
    fn process_inventory_item() {
        let owner = Uuid::new_v4();
        let root = Uuid::new_v4();
        let mut reader = IarReader::new(owner, root, false);
        let mut importer = MockImporter::new();
        let users = MockUsers;

        let item_xml = format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<InventoryItem>
  <Name>Test Box</Name>
  <ID>aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa</ID>
  <InvType>6</InvType>
  <CreatorUUID>bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb</CreatorUUID>
  <CreationDate>1700000000</CreationDate>
  <Owner>{owner}</Owner>
  <Description>test</Description>
  <AssetType>6</AssetType>
  <AssetID>cccccccc-cccc-cccc-cccc-cccccccccccc</AssetID>
  <SaleType>0</SaleType>
  <SalePrice>0</SalePrice>
  <BasePermissions>2147483647</BasePermissions>
  <CurrentPermissions>2147483647</CurrentPermissions>
  <EveryOnePermissions>0</EveryOnePermissions>
  <NextPermissions>2147483647</NextPermissions>
  <Flags>0</Flags>
  <GroupID>00000000-0000-0000-0000-000000000000</GroupID>
  <GroupOwned>False</GroupOwned>
</InventoryItem>"#
        );

        let path = "inventory/Objects__dddddddd-dddd-dddd-dddd-dddddddddddd/Test Box__aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa.xml";

        reader
            .process_entry(path, item_xml.as_bytes(), false, &mut importer, &users)
            .unwrap();

        // Should have created a folder and added an item
        assert_eq!(importer.folders.len(), 1);
        assert_eq!(importer.folders[0].0, "Objects");
        assert_eq!(importer.items.len(), 1);
        assert_eq!(importer.items[0].name, "Test Box");
        assert_eq!(importer.items[0].owner, owner);
        assert_eq!(reader.stats.successful_item_restores, 1);
    }

    #[test]
    fn process_asset() {
        let owner = Uuid::new_v4();
        let root = Uuid::new_v4();
        let mut reader = IarReader::new(owner, root, false);
        let mut importer = MockImporter::new();
        let users = MockUsers;

        let asset_data = b"fake texture data";
        let path = "assets/eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee_texture.jp2";

        reader
            .process_entry(path, asset_data, false, &mut importer, &users)
            .unwrap();

        assert_eq!(importer.assets.len(), 1);
        assert_eq!(
            importer.assets[0].0,
            Uuid::parse_str("eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee").unwrap()
        );
        assert_eq!(reader.stats.successful_asset_restores, 1);
    }

    #[test]
    fn oversized_asset_skipped() {
        let owner = Uuid::new_v4();
        let root = Uuid::new_v4();
        let mut reader = IarReader::new(owner, root, false);
        let mut importer = MockImporter::new();
        let users = MockUsers;

        // Create data exceeding the limit
        let big_data = vec![0u8; MAX_ASSET_SIZE + 1];
        let path = "assets/ffffffff-ffff-ffff-ffff-ffffffffffff_texture.jp2";

        // Should not error, but should skip the asset
        reader
            .process_entry(path, &big_data, false, &mut importer, &users)
            .unwrap();

        assert_eq!(importer.assets.len(), 0);
        assert_eq!(reader.stats.failed_asset_restores, 1);
    }

    #[test]
    fn path_traversal_rejected() {
        let owner = Uuid::new_v4();
        let root = Uuid::new_v4();
        let mut reader = IarReader::new(owner, root, false);
        let mut importer = MockImporter::new();
        let users = MockUsers;

        let result = reader.process_entry(
            "inventory/../../../etc/passwd",
            b"evil",
            false,
            &mut importer,
            &users,
        );
        assert!(result.is_err());
    }

    #[test]
    fn finalize_resolves_links() {
        let owner = Uuid::new_v4();
        let root = Uuid::new_v4();
        let mut reader = IarReader::new(owner, root, false);
        let mut importer = MockImporter::new();
        let users = MockUsers;

        // Simulate loading a normal item first
        let item_uuid = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
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

        // Now load a link item pointing to the real item
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
        assert_eq!(importer.items.len(), 1); // only the real item
        assert_eq!(reader.deferred_links.len(), 1);

        // Finalize resolves links
        reader.finalize(&mut importer).unwrap();

        assert_eq!(importer.items.len(), 2); // real + link
        assert_eq!(reader.stats.successful_item_restores, 2);
        // The link's asset_id should now point to the new ID, not the old one
        let link_item = &importer.items[1];
        assert_ne!(link_item.asset_id, item_uuid);
    }
}
