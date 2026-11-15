// SPDX-License-Identifier: GPL-3.0-or-later

//! Avatar attachments module.
//!
//! Tracks which inventory items are worn on which attachment points for each
//! agent in the region, and handles attach/detach lifecycle around login,
//! logout, and cross-region transfers.

use std::collections::HashMap;

use uuid::Uuid;

use crate::module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
use crate::registry::SceneRef;

// ---------------------------------------------------------------------------
// AttachmentPoint
// ---------------------------------------------------------------------------

/// Attachment-point identifiers matching the Second Life / OpenSimulator
/// protocol values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum AttachmentPoint {
    Default = 0,
    Chest = 1,
    Head = 2,
    LeftShoulder = 3,
    RightShoulder = 4,
    LeftHand = 5,
    RightHand = 6,
    LeftFoot = 7,
    RightFoot = 8,
    Back = 9,
    Pelvis = 10,
    Mouth = 11,
    Chin = 12,
    LeftEar = 13,
    RightEar = 14,
    LeftEye = 15,
    RightEye = 16,
    Nose = 17,
    RUArm = 18,
    RLArm = 19,
    LUArm = 20,
    LLArm = 21,
    RHip = 22,
    RULeg = 23,
    RLLeg = 24,
    LHip = 25,
    LULeg = 26,
    LLLeg = 27,
    Stomach = 28,
    LeftPec = 29,
    RightPec = 30,
    HudCenter2 = 31,
    HudTopRight = 32,
    HudTop = 33,
    HudTopLeft = 34,
    HudCenter = 35,
    HudBottomLeft = 36,
    HudBottom = 37,
    HudBottomRight = 38,
    Neck = 39,
    AvatarCenter = 40,
}

// ---------------------------------------------------------------------------
// AttachmentInfo
// ---------------------------------------------------------------------------

/// Describes a single attached object on an avatar.
#[derive(Debug, Clone)]
pub struct AttachmentInfo {
    /// The inventory item backing this attachment.
    pub item_id: Uuid,
    /// The in-scene object id of the rezzed attachment.
    pub object_id: Uuid,
    /// Where on the avatar this object is worn.
    pub point: AttachmentPoint,
    /// Owner of the attachment (always the wearing agent).
    pub owner_id: Uuid,
}

// ---------------------------------------------------------------------------
// AttachmentsModule
// ---------------------------------------------------------------------------

/// Manages avatar attachments within a region.
///
/// Stub implementation: `wear` creates a placeholder object id and records the
/// attachment; `detach` / `detach_all` remove entries.  Login and logout
/// helpers handle persistence hand-off.
pub struct AttachmentsModule {
    /// Per-agent list of current attachments.
    attachments: HashMap<Uuid, Vec<AttachmentInfo>>,
}

impl AttachmentsModule {
    pub fn new() -> Self {
        Self {
            attachments: HashMap::new(),
        }
    }

    /// Attach an inventory item to the given point.
    ///
    /// Stub: creates a new object id and records the attachment.
    pub fn wear(
        &mut self,
        agent_id: Uuid,
        item_id: Uuid,
        point: AttachmentPoint,
    ) -> Result<Uuid, ModuleError> {
        let object_id = Uuid::new_v4();
        tracing::debug!(
            %agent_id, %item_id, ?point, %object_id,
            "AttachmentsModule: wearing attachment"
        );
        let info = AttachmentInfo {
            item_id,
            object_id,
            point,
            owner_id: agent_id,
        };
        self.attachments.entry(agent_id).or_default().push(info);
        Ok(object_id)
    }

    /// Detach the attachment backed by the given inventory item.
    pub fn detach(&mut self, agent_id: Uuid, item_id: Uuid) -> Result<(), ModuleError> {
        tracing::debug!(%agent_id, %item_id, "AttachmentsModule: detaching");
        if let Some(list) = self.attachments.get_mut(&agent_id) {
            let before = list.len();
            list.retain(|a| a.item_id != item_id);
            if list.len() == before {
                return Err(ModuleError::NotFound(format!(
                    "attachment item {item_id} not found on agent {agent_id}"
                )));
            }
        } else {
            return Err(ModuleError::NotFound(format!(
                "no attachments for agent {agent_id}"
            )));
        }
        Ok(())
    }

    /// Remove all attachments from an agent.
    pub fn detach_all(&mut self, agent_id: Uuid) {
        tracing::debug!(%agent_id, "AttachmentsModule: detaching all");
        self.attachments.remove(&agent_id);
    }

    /// Return the current attachments for an agent.
    pub fn get_attachments(&self, agent_id: Uuid) -> Vec<&AttachmentInfo> {
        self.attachments
            .get(&agent_id)
            .map(|list| list.iter().collect())
            .unwrap_or_default()
    }

    /// Validate that a point value is legal.  Always returns `true` for valid
    /// enum members (the type system enforces this).
    pub fn validate_point(_point: AttachmentPoint) -> bool {
        true
    }

    /// Restore attachments from persistence when an agent logs in.
    pub fn on_agent_login(&mut self, agent_id: Uuid, saved_attachments: Vec<AttachmentInfo>) {
        tracing::debug!(
            %agent_id, count = saved_attachments.len(),
            "AttachmentsModule: restoring attachments on login"
        );
        self.attachments.insert(agent_id, saved_attachments);
    }

    /// Remove and return all attachments for an agent logging out, so the
    /// caller can persist them.
    pub fn on_agent_logout(&mut self, agent_id: Uuid) -> Vec<AttachmentInfo> {
        tracing::debug!(%agent_id, "AttachmentsModule: saving attachments on logout");
        self.attachments.remove(&agent_id).unwrap_or_default()
    }

    /// Clone and return all attachments for cross-region transfer without
    /// removing them from local state.
    pub fn prepare_for_transfer(&self, agent_id: Uuid) -> Vec<AttachmentInfo> {
        tracing::debug!(%agent_id, "AttachmentsModule: preparing attachments for transfer");
        self.attachments.get(&agent_id).cloned().unwrap_or_default()
    }
}

impl Default for AttachmentsModule {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// RegionModule implementation
// ---------------------------------------------------------------------------

impl RegionModule for AttachmentsModule {
    fn name(&self) -> &str {
        "AttachmentsModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::NonShared
    }

    fn initialise(&mut self, _config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        tracing::info!("AttachmentsModule initialised");
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "AttachmentsModule attached to region");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "AttachmentsModule detached from region");
        self.attachments.clear();
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.attachments.clear();
        Ok(())
    }
}
