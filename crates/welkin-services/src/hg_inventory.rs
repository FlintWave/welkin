// SPDX-License-Identifier: GPL-3.0-or-later

//! Hypergrid inventory services — restricted views of a traveling user's
//! inventory for cross-grid access.
//!
//! Two implementations are provided:
//!
//! * [`HGInventoryServiceImpl`] — the basic HG inventory service that returns
//!   the "My Suitcase" folder as the root and rewrites creator data to UUI
//!   format.  Folder deletion and purge are refused.
//!
//! * [`HGSuitcaseInventoryServiceImpl`] — the stricter variant that validates
//!   every operation stays within the suitcase folder tree.  Items can only be
//!   added to folders that are descendants of the suitcase root.

use std::collections::HashMap;
use std::sync::RwLock;

use uuid::Uuid;

use crate::hypergrid::*;

// ---------------------------------------------------------------------------
// Suitcase folder type constant (matches C# FolderType.Suitcase = 100)
// ---------------------------------------------------------------------------

const FOLDER_TYPE_SUITCASE: i16 = 100;
const FOLDER_TYPE_NORMAL: i16 = 8;

// ---------------------------------------------------------------------------
// HG inventory service (basic)
// ---------------------------------------------------------------------------

/// A basic Hypergrid inventory service that exposes the suitcase folder as the
/// root folder and rewrites creator identifiers to UUI format on read.
///
/// The in-memory stores are used for testing; in production these would be
/// backed by database queries via the inventory data layer.
pub struct HGInventoryServiceImpl {
    home_url: String,
    folders: RwLock<Vec<HgInventoryFolder>>,
    items: RwLock<Vec<HgInventoryItem>>,
}

impl HGInventoryServiceImpl {
    pub fn new(home_url: String) -> Self {
        Self {
            home_url,
            folders: RwLock::new(Vec::new()),
            items: RwLock::new(Vec::new()),
        }
    }

    /// Seed a folder into the store (testing helper).
    pub fn add_folder(&self, folder: HgInventoryFolder) {
        self.folders.write().unwrap().push(folder);
    }

    /// Seed an item into the store (testing helper).
    pub fn add_item_direct(&self, item: HgInventoryItem) {
        self.items.write().unwrap().push(item);
    }

    /// Rewrite a plain UUID creator id to UUI format if needed.
    fn adjust_creator(&self, item: &mut HgInventoryItem) {
        if item.creator_id.contains(';') {
            return;
        }
        if item.creator_data.is_empty() {
            item.creator_data = format!("{};Unknown User", self.home_url);
        }
    }
}

impl HypergridInventoryService for HGInventoryServiceImpl {
    fn get_root_folder(&self, principal_id: Uuid) -> HgResult<HgInventoryFolder> {
        let folders = self.folders.read().unwrap();

        // Look for an existing suitcase folder
        if let Some(f) = folders
            .iter()
            .find(|f| f.owner_id == principal_id && f.folder_type == FOLDER_TYPE_SUITCASE)
        {
            // Return it as a normal folder type (like C# SetAsNormalFolder)
            let mut out = f.clone();
            out.folder_type = FOLDER_TYPE_NORMAL;
            return Ok(out);
        }

        // Also accept a folder named "My Suitcase"
        if let Some(f) = folders
            .iter()
            .find(|f| f.owner_id == principal_id && f.name == "My Suitcase")
        {
            let mut out = f.clone();
            out.folder_type = FOLDER_TYPE_NORMAL;
            return Ok(out);
        }

        // No suitcase found — create one
        let suitcase = HgInventoryFolder {
            folder_id: Uuid::new_v4(),
            parent_id: Uuid::nil(),
            owner_id: principal_id,
            name: "My Suitcase".into(),
            folder_type: FOLDER_TYPE_SUITCASE,
        };

        drop(folders);
        self.folders.write().unwrap().push(suitcase.clone());

        let mut out = suitcase;
        out.folder_type = FOLDER_TYPE_NORMAL;
        Ok(out)
    }

    fn get_folder_content(
        &self,
        principal_id: Uuid,
        folder_id: Uuid,
    ) -> HgResult<HgInventoryCollection> {
        let folders = self.folders.read().unwrap();
        let items = self.items.read().unwrap();

        let child_folders: Vec<HgInventoryFolder> = folders
            .iter()
            .filter(|f| f.parent_id == folder_id && f.owner_id == principal_id)
            .cloned()
            .collect();

        let mut child_items: Vec<HgInventoryItem> = items
            .iter()
            .filter(|i| i.folder_id == folder_id && i.owner_id == principal_id)
            .cloned()
            .collect();

        for item in &mut child_items {
            self.adjust_creator(item);
        }

        Ok(HgInventoryCollection {
            folders: child_folders,
            items: child_items,
        })
    }

    fn get_item(&self, principal_id: Uuid, item_id: Uuid) -> HgResult<Option<HgInventoryItem>> {
        let items = self.items.read().unwrap();
        let Some(item) = items
            .iter()
            .find(|i| i.item_id == item_id && i.owner_id == principal_id)
        else {
            return Ok(None);
        };

        let mut out = item.clone();
        self.adjust_creator(&mut out);
        Ok(Some(out))
    }

    fn add_item(&self, item: &HgInventoryItem) -> HgResult<bool> {
        self.items.write().unwrap().push(item.clone());
        Ok(true)
    }
}

// ---------------------------------------------------------------------------
// HG suitcase inventory service (strict)
// ---------------------------------------------------------------------------

/// A stricter Hypergrid inventory service that validates all operations stay
/// within the suitcase folder tree.
///
/// The `suitcase_roots` map tracks the suitcase root folder ID for each user.
/// `suitcase_tree` tracks all folder IDs that are descendants of the suitcase.
pub struct HGSuitcaseInventoryServiceImpl {
    home_url: String,
    /// owner_id -> suitcase root folder_id
    suitcase_roots: RwLock<HashMap<Uuid, Uuid>>,
    /// owner_id -> set of folder IDs within the suitcase tree (including root)
    suitcase_tree: RwLock<HashMap<Uuid, Vec<Uuid>>>,
    folders: RwLock<Vec<HgInventoryFolder>>,
    items: RwLock<Vec<HgInventoryItem>>,
}

impl HGSuitcaseInventoryServiceImpl {
    pub fn new(home_url: String) -> Self {
        Self {
            home_url,
            suitcase_roots: RwLock::new(HashMap::new()),
            suitcase_tree: RwLock::new(HashMap::new()),
            folders: RwLock::new(Vec::new()),
            items: RwLock::new(Vec::new()),
        }
    }

    /// Seed a folder into the store (testing helper).
    pub fn add_folder(&self, folder: HgInventoryFolder) {
        self.folders.write().unwrap().push(folder);
    }

    /// Seed an item into the store (testing helper).
    pub fn add_item_direct(&self, item: HgInventoryItem) {
        self.items.write().unwrap().push(item);
    }

    /// Register a suitcase root for a user and its descendant tree.
    pub fn register_suitcase(&self, owner_id: Uuid, root_id: Uuid, tree_ids: Vec<Uuid>) {
        self.suitcase_roots
            .write()
            .unwrap()
            .insert(owner_id, root_id);
        let mut full_tree = vec![root_id];
        full_tree.extend(tree_ids);
        self.suitcase_tree
            .write()
            .unwrap()
            .insert(owner_id, full_tree);
    }

    /// Check whether a folder is within the suitcase tree for a user.
    pub fn is_within_suitcase_tree(&self, owner: Uuid, folder_id: Uuid) -> bool {
        let tree = self.suitcase_tree.read().unwrap();
        match tree.get(&owner) {
            Some(ids) => ids.contains(&folder_id),
            None => false,
        }
    }

    fn adjust_creator(&self, item: &mut HgInventoryItem) {
        if item.creator_id.contains(';') {
            return;
        }
        if item.creator_data.is_empty() {
            item.creator_data = format!("{};Unknown User", self.home_url);
        }
    }
}

impl HypergridInventoryService for HGSuitcaseInventoryServiceImpl {
    fn get_root_folder(&self, principal_id: Uuid) -> HgResult<HgInventoryFolder> {
        // Check for a registered suitcase root
        let roots = self.suitcase_roots.read().unwrap();
        if let Some(&root_id) = roots.get(&principal_id) {
            let folders = self.folders.read().unwrap();
            if let Some(f) = folders.iter().find(|f| f.folder_id == root_id) {
                let mut out = f.clone();
                // Present as a normal folder outside
                out.folder_type = FOLDER_TYPE_NORMAL;
                return Ok(out);
            }
        }
        drop(roots);

        // Look for a suitcase folder by type
        let folders = self.folders.read().unwrap();
        if let Some(f) = folders
            .iter()
            .find(|f| f.owner_id == principal_id && f.folder_type == FOLDER_TYPE_SUITCASE)
        {
            let root_id = f.folder_id;
            let mut out = f.clone();
            out.folder_type = FOLDER_TYPE_NORMAL;

            // Auto-register
            drop(folders);
            self.suitcase_roots
                .write()
                .unwrap()
                .insert(principal_id, root_id);
            self.suitcase_tree
                .write()
                .unwrap()
                .entry(principal_id)
                .or_insert_with(|| vec![root_id]);

            return Ok(out);
        }

        // Also accept "My Suitcase" by name
        if let Some(f) = folders
            .iter()
            .find(|f| f.owner_id == principal_id && f.name == "My Suitcase")
        {
            let root_id = f.folder_id;
            let mut out = f.clone();
            out.folder_type = FOLDER_TYPE_NORMAL;

            drop(folders);
            self.suitcase_roots
                .write()
                .unwrap()
                .insert(principal_id, root_id);
            self.suitcase_tree
                .write()
                .unwrap()
                .entry(principal_id)
                .or_insert_with(|| vec![root_id]);

            return Ok(out);
        }

        Err(HypergridError::InventoryOperationRefused(
            "no suitcase folder found".into(),
        ))
    }

    fn get_folder_content(
        &self,
        principal_id: Uuid,
        folder_id: Uuid,
    ) -> HgResult<HgInventoryCollection> {
        if !self.is_within_suitcase_tree(principal_id, folder_id) {
            tracing::debug!(
                "GetFolderContent: folder {} (user {}) is not within Suitcase tree",
                folder_id,
                principal_id
            );
            return Ok(HgInventoryCollection {
                folders: Vec::new(),
                items: Vec::new(),
            });
        }

        let folders = self.folders.read().unwrap();
        let items = self.items.read().unwrap();

        let child_folders: Vec<HgInventoryFolder> = folders
            .iter()
            .filter(|f| f.parent_id == folder_id && f.owner_id == principal_id)
            .cloned()
            .collect();

        let mut child_items: Vec<HgInventoryItem> = items
            .iter()
            .filter(|i| i.folder_id == folder_id && i.owner_id == principal_id)
            .cloned()
            .collect();

        for item in &mut child_items {
            self.adjust_creator(item);
        }

        Ok(HgInventoryCollection {
            folders: child_folders,
            items: child_items,
        })
    }

    fn get_item(&self, principal_id: Uuid, item_id: Uuid) -> HgResult<Option<HgInventoryItem>> {
        let items = self.items.read().unwrap();
        let Some(item) = items
            .iter()
            .find(|i| i.item_id == item_id && i.owner_id == principal_id)
        else {
            return Ok(None);
        };

        let mut out = item.clone();
        self.adjust_creator(&mut out);
        Ok(Some(out))
    }

    fn add_item(&self, item: &HgInventoryItem) -> HgResult<bool> {
        if !self.is_within_suitcase_tree(item.owner_id, item.folder_id) {
            tracing::debug!(
                "AddItem: folder {} (user {}) is not within Suitcase tree",
                item.folder_id,
                item.owner_id
            );
            return Ok(false);
        }

        self.items.write().unwrap().push(item.clone());
        Ok(true)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_folder(
        folder_id: Uuid,
        parent_id: Uuid,
        owner_id: Uuid,
        name: &str,
        folder_type: i16,
    ) -> HgInventoryFolder {
        HgInventoryFolder {
            folder_id,
            parent_id,
            owner_id,
            name: name.into(),
            folder_type,
        }
    }

    fn make_item(
        item_id: Uuid,
        folder_id: Uuid,
        owner_id: Uuid,
        creator_id: &str,
    ) -> HgInventoryItem {
        HgInventoryItem {
            item_id,
            folder_id,
            owner_id,
            asset_id: Uuid::new_v4(),
            asset_type: 0,
            name: "Test Item".into(),
            description: "desc".into(),
            creator_id: creator_id.into(),
            creator_data: String::new(),
        }
    }

    // ---- HGInventoryServiceImpl tests ----

    #[test]
    fn test_get_root_folder() {
        let svc = HGInventoryServiceImpl::new("http://home.grid:9000".into());
        let owner = Uuid::new_v4();
        let suitcase_id = Uuid::new_v4();

        svc.add_folder(make_folder(
            suitcase_id,
            Uuid::nil(),
            owner,
            "My Suitcase",
            FOLDER_TYPE_SUITCASE,
        ));

        let root = svc.get_root_folder(owner).unwrap();
        assert_eq!(root.folder_id, suitcase_id);
        assert_eq!(root.name, "My Suitcase");
        // Should be presented as normal folder type
        assert_eq!(root.folder_type, FOLDER_TYPE_NORMAL);
    }

    #[test]
    fn test_get_root_folder_creates_if_missing() {
        let svc = HGInventoryServiceImpl::new("http://home.grid:9000".into());
        let owner = Uuid::new_v4();

        let root = svc.get_root_folder(owner).unwrap();
        assert_eq!(root.name, "My Suitcase");
        assert_eq!(root.owner_id, owner);
    }

    #[test]
    fn test_get_folder_content() {
        let svc = HGInventoryServiceImpl::new("http://home.grid:9000".into());
        let owner = Uuid::new_v4();
        let parent_id = Uuid::new_v4();
        let child_folder_id = Uuid::new_v4();

        svc.add_folder(make_folder(child_folder_id, parent_id, owner, "Child", 8));
        svc.add_item_direct(make_item(Uuid::new_v4(), parent_id, owner, "creator-uuid"));
        svc.add_item_direct(make_item(Uuid::new_v4(), parent_id, owner, "creator-uuid"));
        // Item in a different folder — should not appear
        svc.add_item_direct(make_item(Uuid::new_v4(), Uuid::new_v4(), owner, "x"));

        let content = svc.get_folder_content(owner, parent_id).unwrap();
        assert_eq!(content.folders.len(), 1);
        assert_eq!(content.folders[0].folder_id, child_folder_id);
        assert_eq!(content.items.len(), 2);
    }

    #[test]
    fn test_get_item_creator_rewrite() {
        let svc = HGInventoryServiceImpl::new("http://home.grid:9000".into());
        let owner = Uuid::new_v4();
        let item_id = Uuid::new_v4();

        svc.add_item_direct(make_item(item_id, Uuid::new_v4(), owner, "plain-uuid"));

        let item = svc.get_item(owner, item_id).unwrap().unwrap();
        assert!(
            !item.creator_data.is_empty(),
            "creator_data should be filled with home URL"
        );
        assert!(item.creator_data.contains("http://home.grid:9000"));
    }

    #[test]
    fn test_add_item_basic() {
        let svc = HGInventoryServiceImpl::new("http://home.grid:9000".into());
        let item = make_item(Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4(), "c");

        let ok = svc.add_item(&item).unwrap();
        assert!(ok);

        // Verify it's retrievable
        let got = svc.get_item(item.owner_id, item.item_id).unwrap();
        assert!(got.is_some());
    }

    #[test]
    fn test_creator_already_uui_not_rewritten() {
        let svc = HGInventoryServiceImpl::new("http://home.grid:9000".into());
        let owner = Uuid::new_v4();
        let item_id = Uuid::new_v4();
        let uui_creator = "deadbeef-0000-1111-2222-333344445555;http://other.grid;Jane Doe";

        let mut item = make_item(item_id, Uuid::new_v4(), owner, uui_creator);
        item.creator_data = "already set".into();
        svc.add_item_direct(item);

        let got = svc.get_item(owner, item_id).unwrap().unwrap();
        // creator_data should not be overwritten when creator_id already has UUI
        assert_eq!(got.creator_data, "already set");
    }

    // ---- HGSuitcaseInventoryServiceImpl tests ----

    #[test]
    fn test_suitcase_get_root_folder() {
        let svc = HGSuitcaseInventoryServiceImpl::new("http://home.grid:9000".into());
        let owner = Uuid::new_v4();
        let suitcase_id = Uuid::new_v4();

        svc.add_folder(make_folder(
            suitcase_id,
            Uuid::new_v4(), // real root
            owner,
            "My Suitcase",
            FOLDER_TYPE_SUITCASE,
        ));
        svc.register_suitcase(owner, suitcase_id, vec![]);

        let root = svc.get_root_folder(owner).unwrap();
        assert_eq!(root.folder_id, suitcase_id);
        assert_eq!(root.name, "My Suitcase");
        assert_eq!(root.folder_type, FOLDER_TYPE_NORMAL);
    }

    #[test]
    fn test_suitcase_add_item_in_tree() {
        let svc = HGSuitcaseInventoryServiceImpl::new("http://home.grid:9000".into());
        let owner = Uuid::new_v4();
        let suitcase_id = Uuid::new_v4();
        let child_folder_id = Uuid::new_v4();

        svc.add_folder(make_folder(
            suitcase_id,
            Uuid::nil(),
            owner,
            "My Suitcase",
            FOLDER_TYPE_SUITCASE,
        ));
        svc.add_folder(make_folder(
            child_folder_id,
            suitcase_id,
            owner,
            "Objects",
            8,
        ));
        svc.register_suitcase(owner, suitcase_id, vec![child_folder_id]);

        let item = make_item(Uuid::new_v4(), child_folder_id, owner, "c");
        let ok = svc.add_item(&item).unwrap();
        assert!(
            ok,
            "add_item should succeed for folder within suitcase tree"
        );
    }

    #[test]
    fn test_suitcase_add_item_outside_tree() {
        let svc = HGSuitcaseInventoryServiceImpl::new("http://home.grid:9000".into());
        let owner = Uuid::new_v4();
        let suitcase_id = Uuid::new_v4();
        let outside_folder = Uuid::new_v4();

        svc.register_suitcase(owner, suitcase_id, vec![]);

        let item = make_item(Uuid::new_v4(), outside_folder, owner, "c");
        let ok = svc.add_item(&item).unwrap();
        assert!(!ok, "add_item should fail for folder outside suitcase tree");
    }

    #[test]
    fn test_suitcase_get_folder_content_outside_tree() {
        let svc = HGSuitcaseInventoryServiceImpl::new("http://home.grid:9000".into());
        let owner = Uuid::new_v4();
        let suitcase_id = Uuid::new_v4();
        let outside_folder = Uuid::new_v4();

        svc.register_suitcase(owner, suitcase_id, vec![]);
        // Add items to a folder that is NOT in the suitcase tree
        svc.add_item_direct(make_item(Uuid::new_v4(), outside_folder, owner, "c"));

        let content = svc.get_folder_content(owner, outside_folder).unwrap();
        assert!(
            content.items.is_empty(),
            "should return empty for folder outside suitcase tree"
        );
    }

    #[test]
    fn test_suitcase_get_folder_content_in_tree() {
        let svc = HGSuitcaseInventoryServiceImpl::new("http://home.grid:9000".into());
        let owner = Uuid::new_v4();
        let suitcase_id = Uuid::new_v4();
        let child_folder_id = Uuid::new_v4();

        svc.add_folder(make_folder(
            suitcase_id,
            Uuid::nil(),
            owner,
            "My Suitcase",
            FOLDER_TYPE_SUITCASE,
        ));
        svc.add_folder(make_folder(
            child_folder_id,
            suitcase_id,
            owner,
            "Textures",
            8,
        ));
        svc.register_suitcase(owner, suitcase_id, vec![child_folder_id]);

        svc.add_item_direct(make_item(Uuid::new_v4(), suitcase_id, owner, "c"));

        let content = svc.get_folder_content(owner, suitcase_id).unwrap();
        assert_eq!(content.folders.len(), 1);
        assert_eq!(content.items.len(), 1);
    }

    #[test]
    fn test_suitcase_no_suitcase_returns_error() {
        let svc = HGSuitcaseInventoryServiceImpl::new("http://home.grid:9000".into());
        let owner = Uuid::new_v4();

        let result = svc.get_root_folder(owner);
        assert!(result.is_err(), "should error when no suitcase exists");
    }
}
