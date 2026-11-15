// SPDX-License-Identifier: GPL-3.0-or-later

//! Hypergrid asset service — a filtered wrapper around the local asset store
//! that enforces per-type export/import permissions and rewrites creator
//! identifiers to Universal User Identifier (UUI) format for cross-grid
//! attribution.

use std::collections::HashMap;
use std::sync::RwLock;

use uuid::Uuid;

use crate::hypergrid::*;

// ---------------------------------------------------------------------------
// Asset permissions
// ---------------------------------------------------------------------------

/// Configurable per-type allow lists for asset transfer across grids.
///
/// An empty list means "allow nothing" — every type must be explicitly listed.
pub struct AssetPermissions {
    pub allowed_export_types: Vec<i8>,
    pub allowed_import_types: Vec<i8>,
}

impl AssetPermissions {
    pub fn new(export_types: Vec<i8>, import_types: Vec<i8>) -> Self {
        Self {
            allowed_export_types: export_types,
            allowed_import_types: import_types,
        }
    }

    /// Returns `true` if the given asset type may be sent to a foreign grid.
    pub fn allowed_export(&self, asset_type: i8) -> bool {
        self.allowed_export_types.contains(&asset_type)
    }

    /// Returns `true` if the given asset type may be received from a foreign grid.
    pub fn allowed_import(&self, asset_type: i8) -> bool {
        self.allowed_import_types.contains(&asset_type)
    }
}

// ---------------------------------------------------------------------------
// HG asset service implementation
// ---------------------------------------------------------------------------

/// A Hypergrid-filtered asset service backed by an in-memory store.
///
/// In production the backing store would be an `Arc<dyn AssetService>`;
/// for testing we use a plain `HashMap` behind a `RwLock`.
pub struct HGAssetServiceImpl {
    home_url: String,
    perms: AssetPermissions,
    assets: RwLock<HashMap<Uuid, HgAsset>>,
}

impl HGAssetServiceImpl {
    pub fn new(home_url: String, perms: AssetPermissions) -> Self {
        Self {
            home_url,
            perms,
            assets: RwLock::new(HashMap::new()),
        }
    }

    /// Seed the backing store (used by tests).
    pub fn insert_local_asset(&self, asset: HgAsset) {
        self.assets.write().unwrap().insert(asset.id, asset);
    }

    /// Rewrite a plain UUID creator id to UUI format:
    ///   `uuid;home_url;FirstName LastName`
    ///
    /// If the creator id already contains a semicolon it is assumed to be in
    /// UUI format already and is returned unchanged.
    fn adjust_creator_id(&self, creator_id: &str) -> String {
        if creator_id.contains(';') {
            return creator_id.to_string();
        }
        // The C# code looks up the user account to get FirstName/LastName.
        // We don't have access to an account service here, so we use
        // "Unknown User" as a placeholder — the important part is the
        // home_url which lets the receiving grid resolve the identity.
        format!("{};{};Unknown User", creator_id, self.home_url)
    }
}

impl HypergridAssetService for HGAssetServiceImpl {
    fn get_asset(&self, asset_id: Uuid) -> HgResult<Option<HgAsset>> {
        let store = self.assets.read().unwrap();
        let Some(asset) = store.get(&asset_id) else {
            return Ok(None);
        };

        if !self.perms.allowed_export(asset.asset_type) {
            return Ok(None);
        }

        let mut out = asset.clone();
        out.creator_id = self.adjust_creator_id(&out.creator_id);
        Ok(Some(out))
    }

    fn get_metadata(&self, asset_id: Uuid) -> HgResult<Option<HgAssetMetadata>> {
        let store = self.assets.read().unwrap();
        let Some(asset) = store.get(&asset_id) else {
            return Ok(None);
        };

        if !self.perms.allowed_export(asset.asset_type) {
            return Ok(None);
        }

        Ok(Some(HgAssetMetadata {
            id: asset.id,
            asset_type: asset.asset_type,
            name: asset.name.clone(),
            description: asset.description.clone(),
            creator_id: self.adjust_creator_id(&asset.creator_id),
        }))
    }

    fn store_asset(&self, asset: HgAsset) -> HgResult<Option<Uuid>> {
        if !self.perms.allowed_import(asset.asset_type) {
            return Ok(None);
        }

        let id = asset.id;
        self.assets.write().unwrap().insert(id, asset);
        Ok(Some(id))
    }

    fn assets_exist(&self, asset_ids: &[Uuid]) -> HgResult<Vec<bool>> {
        let store = self.assets.read().unwrap();
        Ok(asset_ids.iter().map(|id| store.contains_key(id)).collect())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Asset type constants matching SL conventions.
    const TEXTURE: i8 = 0;
    const SOUND: i8 = 1;
    const SCRIPT: i8 = 10;
    const OBJECT: i8 = 6;

    fn make_service() -> HGAssetServiceImpl {
        HGAssetServiceImpl::new(
            "http://mygrid.example:9000".to_string(),
            AssetPermissions::new(
                vec![TEXTURE, SOUND, OBJECT], // allowed export
                vec![TEXTURE, SOUND],         // allowed import
            ),
        )
    }

    fn make_asset(id: Uuid, asset_type: i8, creator: &str) -> HgAsset {
        HgAsset {
            id,
            asset_type,
            name: "test asset".into(),
            description: "desc".into(),
            creator_id: creator.into(),
            data: vec![1, 2, 3],
            local: false,
            temporary: false,
        }
    }

    #[test]
    fn test_export_allowed() {
        let svc = make_service();
        let id = Uuid::new_v4();
        svc.insert_local_asset(make_asset(id, TEXTURE, "creator-uuid"));

        let result = svc.get_asset(id).unwrap();
        assert!(
            result.is_some(),
            "asset with allowed export type should be returned"
        );
    }

    #[test]
    fn test_export_denied() {
        let svc = make_service();
        let id = Uuid::new_v4();
        svc.insert_local_asset(make_asset(id, SCRIPT, "creator-uuid"));

        let result = svc.get_asset(id).unwrap();
        assert!(
            result.is_none(),
            "asset with denied export type should not be returned"
        );
    }

    #[test]
    fn test_import_allowed() {
        let svc = make_service();
        let asset = make_asset(Uuid::new_v4(), TEXTURE, "foreign-creator");
        let id = asset.id;

        let result = svc.store_asset(asset).unwrap();
        assert_eq!(result, Some(id));
    }

    #[test]
    fn test_import_denied() {
        let svc = make_service();
        let asset = make_asset(Uuid::new_v4(), SCRIPT, "foreign-creator");

        let result = svc.store_asset(asset).unwrap();
        assert!(
            result.is_none(),
            "asset with denied import type should not be stored"
        );
    }

    #[test]
    fn test_creator_rewrite() {
        let svc = make_service();
        let id = Uuid::new_v4();
        let plain_uuid = "deadbeef-0000-1111-2222-333344445555";
        svc.insert_local_asset(make_asset(id, TEXTURE, plain_uuid));

        let asset = svc.get_asset(id).unwrap().unwrap();
        assert!(
            asset.creator_id.contains(';'),
            "creator_id should be rewritten to UUI format"
        );
        assert!(
            asset.creator_id.starts_with(plain_uuid),
            "UUI should start with the original UUID"
        );
        assert!(
            asset.creator_id.contains("http://mygrid.example:9000"),
            "UUI should contain the home URL"
        );
    }

    #[test]
    fn test_creator_already_uui_unchanged() {
        let svc = make_service();
        let id = Uuid::new_v4();
        let uui = "deadbeef-0000-1111-2222-333344445555;http://other.grid:9000;Jane Doe";
        svc.insert_local_asset(make_asset(id, TEXTURE, uui));

        let asset = svc.get_asset(id).unwrap().unwrap();
        assert_eq!(
            asset.creator_id, uui,
            "already-UUI creator should not be rewritten"
        );
    }

    #[test]
    fn test_assets_exist() {
        let svc = make_service();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id_missing = Uuid::new_v4();
        svc.insert_local_asset(make_asset(id1, TEXTURE, "c"));
        svc.insert_local_asset(make_asset(id2, SOUND, "c"));

        let result = svc.assets_exist(&[id1, id_missing, id2]).unwrap();
        assert_eq!(result, vec![true, false, true]);
    }

    #[test]
    fn test_get_metadata_export_denied() {
        let svc = make_service();
        let id = Uuid::new_v4();
        svc.insert_local_asset(make_asset(id, SCRIPT, "creator"));

        let result = svc.get_metadata(id).unwrap();
        assert!(
            result.is_none(),
            "metadata for denied export type should be None"
        );
    }

    #[test]
    fn test_get_metadata_creator_rewrite() {
        let svc = make_service();
        let id = Uuid::new_v4();
        let plain = "aaaabbbb-cccc-dddd-eeee-ffffffffffff";
        svc.insert_local_asset(make_asset(id, TEXTURE, plain));

        let meta = svc.get_metadata(id).unwrap().unwrap();
        assert!(meta.creator_id.contains(';'));
        assert!(meta.creator_id.starts_with(plain));
    }
}
