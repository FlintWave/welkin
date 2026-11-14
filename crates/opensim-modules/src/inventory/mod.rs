// SPDX-License-Identifier: GPL-3.0-or-later

//! Inventory transfer, access, asset transactions, and xfer modules.

use std::collections::HashMap;

use uuid::Uuid;

use crate::module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
use crate::registry::SceneRef;

// ---------------------------------------------------------------------------
// AssetType
// ---------------------------------------------------------------------------

/// The type of asset being uploaded or transferred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssetType {
    Texture,
    Sound,
    Animation,
    Clothing,
    Bodypart,
    Script,
    Object,
    Notecard,
    Landmark,
    Gesture,
}

// ---------------------------------------------------------------------------
// RezResult / DerezAction
// ---------------------------------------------------------------------------

/// Outcome of a rez-object operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RezResult {
    /// The object was successfully rezzed into the scene.
    Success { object_id: Uuid },
    /// The agent does not have permission to rez this item.
    PermissionDenied,
    /// The inventory item was not found.
    ItemNotFound,
    /// An unspecified failure.
    Failed(String),
}

/// What to do when de-rezzing an object from the scene.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DerezAction {
    /// Take the object into the agent's inventory.
    Take,
    /// Take a copy, leaving the original in-world.
    TakeCopy,
    /// Return the object to its owner's inventory.
    Return,
    /// Delete the object permanently.
    Delete,
}

// ---------------------------------------------------------------------------
// UploadTransaction
// ---------------------------------------------------------------------------

/// Tracks an in-progress asset upload.
#[derive(Debug, Clone)]
pub struct UploadTransaction {
    pub transaction_id: Uuid,
    pub agent_id: Uuid,
    pub asset_type: AssetType,
    pub data: Vec<u8>,
    pub created: i64,
}

// ---------------------------------------------------------------------------
// InventoryTransferModule
// ---------------------------------------------------------------------------

/// Handles agent-to-agent inventory item and folder transfers.
///
/// Stub: real implementation would integrate with the inventory service and
/// instant-message transport for offer/accept/decline flows.
pub struct InventoryTransferModule;

impl InventoryTransferModule {
    pub fn new() -> Self {
        Self
    }

    /// Give an inventory item from one agent to another.
    ///
    /// Returns the new item UUID in the recipient's inventory.
    pub fn give_item(
        &self,
        from_agent: Uuid,
        to_agent: Uuid,
        item_id: Uuid,
    ) -> Result<Uuid, ModuleError> {
        tracing::debug!(
            %from_agent, %to_agent, %item_id,
            "give_item: stub — creating placeholder item"
        );
        Ok(Uuid::new_v4())
    }

    /// Give an inventory folder from one agent to another.
    ///
    /// Returns the new folder UUID in the recipient's inventory.
    pub fn give_folder(
        &self,
        from_agent: Uuid,
        to_agent: Uuid,
        folder_id: Uuid,
    ) -> Result<Uuid, ModuleError> {
        tracing::debug!(
            %from_agent, %to_agent, %folder_id,
            "give_folder: stub — creating placeholder folder"
        );
        Ok(Uuid::new_v4())
    }

    /// Accept a pending inventory gift.
    pub fn accept_gift(
        &self,
        agent_id: Uuid,
        transaction_id: Uuid,
    ) -> Result<(), ModuleError> {
        tracing::debug!(
            %agent_id, %transaction_id,
            "accept_gift: stub — gift accepted"
        );
        Ok(())
    }

    /// Decline a pending inventory gift.
    pub fn decline_gift(
        &self,
        agent_id: Uuid,
        transaction_id: Uuid,
    ) -> Result<(), ModuleError> {
        tracing::debug!(
            %agent_id, %transaction_id,
            "decline_gift: stub — gift declined"
        );
        Ok(())
    }
}

impl Default for InventoryTransferModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for InventoryTransferModule {
    fn name(&self) -> &str {
        "InventoryTransferModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::NonShared
    }

    fn initialise(&mut self, _config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        tracing::info!("InventoryTransferModule initialised");
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "InventoryTransferModule attached to region");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "InventoryTransferModule detached from region");
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// InventoryAccessModule
// ---------------------------------------------------------------------------

/// Controls rezzing objects from inventory into the scene and de-rezzing
/// objects back into inventory.
///
/// Stub: real implementation would integrate with the inventory and asset
/// services, check permissions, and create/destroy scene objects.
pub struct InventoryAccessModule;

impl InventoryAccessModule {
    pub fn new() -> Self {
        Self
    }

    /// Rez a single object from the agent's inventory into the scene.
    pub fn rez_object(
        &self,
        agent_id: Uuid,
        item_id: Uuid,
        position: [f32; 3],
        rotation: [f32; 4],
    ) -> RezResult {
        tracing::debug!(
            %agent_id, %item_id,
            ?position, ?rotation,
            "rez_object: stub — returning success"
        );
        RezResult::Success {
            object_id: Uuid::new_v4(),
        }
    }

    /// Rez a coalesced (multi-object) inventory item into the scene.
    pub fn rez_coalesced(
        &self,
        agent_id: Uuid,
        item_id: Uuid,
        position: [f32; 3],
    ) -> RezResult {
        tracing::debug!(
            %agent_id, %item_id,
            ?position,
            "rez_coalesced: stub — returning success"
        );
        RezResult::Success {
            object_id: Uuid::new_v4(),
        }
    }

    /// De-rez an object from the scene back into inventory.
    ///
    /// Returns the new inventory item UUID on success.
    pub fn derez_object(
        &self,
        agent_id: Uuid,
        object_id: Uuid,
        action: DerezAction,
    ) -> Result<Uuid, ModuleError> {
        tracing::debug!(
            %agent_id, %object_id, ?action,
            "derez_object: stub — creating placeholder item"
        );
        Ok(Uuid::new_v4())
    }
}

impl Default for InventoryAccessModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for InventoryAccessModule {
    fn name(&self) -> &str {
        "InventoryAccessModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::NonShared
    }

    fn initialise(&mut self, _config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        tracing::info!("InventoryAccessModule initialised");
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "InventoryAccessModule attached to region");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "InventoryAccessModule detached from region");
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// AssetTransactionModule
// ---------------------------------------------------------------------------

/// Manages multi-packet asset uploads from the viewer.
///
/// Stub: real implementation would buffer incoming data, validate the asset,
/// and persist it via the asset service.
pub struct AssetTransactionModule {
    transactions: HashMap<Uuid, UploadTransaction>,
}

impl AssetTransactionModule {
    pub fn new() -> Self {
        Self {
            transactions: HashMap::new(),
        }
    }

    /// Begin a new asset upload.  Returns the transaction UUID.
    pub fn begin_upload(&mut self, agent_id: Uuid, asset_type: AssetType) -> Uuid {
        let transaction_id = Uuid::new_v4();
        tracing::debug!(
            %agent_id, ?asset_type, %transaction_id,
            "begin_upload: stub — transaction created"
        );
        self.transactions.insert(
            transaction_id,
            UploadTransaction {
                transaction_id,
                agent_id,
                asset_type,
                data: Vec::new(),
                created: 0, // stub timestamp
            },
        );
        transaction_id
    }

    /// Append data to an in-progress upload.
    pub fn append_data(
        &mut self,
        transaction_id: Uuid,
        data: &[u8],
    ) -> Result<(), ModuleError> {
        let txn = self.transactions.get_mut(&transaction_id).ok_or_else(|| {
            ModuleError::NotFound(format!("upload transaction {transaction_id}"))
        })?;
        tracing::debug!(
            %transaction_id,
            bytes = data.len(),
            "append_data: stub — appending"
        );
        txn.data.extend_from_slice(data);
        Ok(())
    }

    /// Complete an upload.  Returns the new asset UUID.
    pub fn complete_upload(
        &mut self,
        transaction_id: Uuid,
    ) -> Result<Uuid, ModuleError> {
        let _txn = self.transactions.remove(&transaction_id).ok_or_else(|| {
            ModuleError::NotFound(format!("upload transaction {transaction_id}"))
        })?;
        let asset_id = Uuid::new_v4();
        tracing::debug!(
            %transaction_id, %asset_id,
            "complete_upload: stub — asset created"
        );
        Ok(asset_id)
    }

    /// Cancel and discard an in-progress upload.
    pub fn cancel_upload(&mut self, transaction_id: Uuid) {
        tracing::debug!(%transaction_id, "cancel_upload: stub — transaction removed");
        self.transactions.remove(&transaction_id);
    }

    /// Returns the number of in-progress transactions (useful for tests).
    pub fn transaction_count(&self) -> usize {
        self.transactions.len()
    }
}

impl Default for AssetTransactionModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for AssetTransactionModule {
    fn name(&self) -> &str {
        "AssetTransactionModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::NonShared
    }

    fn initialise(&mut self, _config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        tracing::info!("AssetTransactionModule initialised");
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "AssetTransactionModule attached to region");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "AssetTransactionModule detached from region");
        self.transactions.clear();
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.transactions.clear();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// XferModule
// ---------------------------------------------------------------------------

/// Handles the low-level Xfer (file transfer) protocol.
///
/// Stub: real implementation would manage packet-level transfers between the
/// viewer and simulator.
pub struct XferModule;

impl XferModule {
    pub fn new() -> Self {
        Self
    }

    /// Request a file transfer.  Returns the xfer UUID.
    pub fn request_xfer(&self, filename: &str) -> Uuid {
        let xfer_id = Uuid::new_v4();
        tracing::debug!(%xfer_id, filename, "request_xfer: stub — xfer started");
        xfer_id
    }

    /// Complete a file transfer.  Returns the transferred bytes.
    pub fn complete_xfer(&self, xfer_id: Uuid) -> Result<Vec<u8>, ModuleError> {
        tracing::debug!(%xfer_id, "complete_xfer: stub — returning empty data");
        Ok(Vec::new())
    }
}

impl Default for XferModule {
    fn default() -> Self {
        Self::new()
    }
}
