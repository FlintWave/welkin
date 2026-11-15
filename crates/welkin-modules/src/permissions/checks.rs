// SPDX-License-Identifier: GPL-3.0-or-later

//! Permission-check functions implemented as methods on
//! [`DefaultPermissionsModule`].
//!
//! Each function follows the same pattern from the C# source:
//!
//! 1. If `bypass_permissions` is set, return `bypass_permissions_value`.
//! 2. If `bypass_permissions_godmode` (here `implicit_gods` / `automatic_gods`)
//!    is active and the agent is a god, return `true`.
//! 3. Apply the specific permission logic.

use uuid::Uuid;
use welkin_types::PermissionMask;

use super::{DefaultPermissionsModule, UserSet};
use crate::land::{LandData, ParcelFlags};

/// Simple description of object ownership / group used by permission checks
/// that do not have access to the full scene graph.
#[derive(Debug, Clone)]
pub struct ObjectPerms {
    pub owner_id: Uuid,
    pub group_id: Uuid,
    /// The object's effective owner permission mask.
    pub owner_mask: PermissionMask,
}

impl DefaultPermissionsModule {
    // -----------------------------------------------------------------------
    // Bypass / god short-circuits
    // -----------------------------------------------------------------------

    /// If bypass is on, return the bypass value.  Otherwise `None` — the
    /// caller must continue with the specific check.
    #[inline]
    fn bypass_check(&self) -> Option<bool> {
        if self.config.bypass_permissions {
            Some(self.config.bypass_permissions_value)
        } else {
            None
        }
    }

    /// If the agent is a god and godmode-bypass is in effect, return `true`.
    /// Otherwise `None`.
    #[inline]
    fn god_bypass(&self, agent_id: Uuid) -> Option<bool> {
        // C# checks `sp.IsGod` which requires the user to have requested
        // god-mode.  We approximate this with `is_god()` which already
        // accounts for `automatic_gods` / `implicit_gods`.
        if self.is_god(agent_id) {
            Some(true)
        } else {
            None
        }
    }

    // -----------------------------------------------------------------------
    // Object permissions
    // -----------------------------------------------------------------------

    /// Can the agent rez (create) a new object at the given position?
    ///
    /// Port of `CanRezObject`.
    pub fn can_create_object(
        &self,
        agent_id: Uuid,
        _position: (f32, f32, f32),
        parcel: Option<&LandData>,
    ) -> bool {
        if let Some(v) = self.bypass_check() {
            return v;
        }
        if let Some(v) = self.god_bypass(agent_id) {
            return v;
        }

        let parcel = match parcel {
            Some(p) => p,
            None => return false,
        };

        // Parcel owner can always rez.
        if agent_id == parcel.owner_id {
            return true;
        }

        // Parcel has CreateObjects open to all.
        let flags = ParcelFlags::from_bits_truncate(parcel.flags);
        if flags.contains(ParcelFlags::CREATE_OBJECTS) {
            return true;
        }

        // Admin can always rez.
        if self.is_administrator(agent_id) {
            return true;
        }

        // Group create-objects.
        if !parcel.group_id.is_nil()
            && flags.contains(ParcelFlags::CREATE_GROUP_OBJECTS)
            && agent_id == parcel.group_id
        {
            // Simplified: in full implementation would check group membership.
            return true;
        }

        false
    }

    /// Can the agent delete this object?
    ///
    /// Port of `CanDeleteObject` / `CanDeleteObjectByIDs`.
    pub fn can_delete_object(
        &self,
        agent_id: Uuid,
        object_owner: Uuid,
        object_group: Uuid,
    ) -> bool {
        if let Some(v) = self.bypass_check() {
            return v;
        }
        if let Some(v) = self.god_bypass(agent_id) {
            return v;
        }

        // Owner can always delete.
        if agent_id == object_owner {
            return true;
        }

        // Administrator can delete.
        if self.is_administrator(agent_id) {
            return true;
        }

        // Group member with group-owned object: simplified check.
        if !object_group.is_nil() && object_owner == object_group {
            // In the full implementation this would check group powers.
            // We allow if the agent is in the group (placeholder).
        }

        false
    }

    /// Can the agent edit (modify) this object?
    ///
    /// Port of `CanEditObject`.
    pub fn can_edit_object(
        &self,
        agent_id: Uuid,
        object_owner: Uuid,
        _object_group: Uuid,
        object_perms: PermissionMask,
    ) -> bool {
        if let Some(v) = self.bypass_check() {
            return v;
        }
        if let Some(v) = self.god_bypass(agent_id) {
            return v;
        }

        // Owner with MODIFY bit.
        if agent_id == object_owner {
            return object_perms.contains(PermissionMask::MODIFY);
        }

        // Administrator can always edit.
        if self.is_administrator(agent_id) {
            return true;
        }

        false
    }

    /// Can the agent move this object?
    ///
    /// Port of `CanMoveObject`.
    pub fn can_move_object(
        &self,
        agent_id: Uuid,
        object_owner: Uuid,
        _object_group: Uuid,
        object_perms: PermissionMask,
    ) -> bool {
        if let Some(v) = self.bypass_check() {
            return v;
        }
        if let Some(v) = self.god_bypass(agent_id) {
            return v;
        }

        // Owner with MOVE bit.
        if agent_id == object_owner {
            return object_perms.contains(PermissionMask::MOVE);
        }

        // Administrator.
        if self.is_administrator(agent_id) {
            return true;
        }

        false
    }

    /// Can the agent copy this object?
    ///
    /// Port of `CanDuplicateObject` / `CanTakeCopyObject`.
    pub fn can_copy_object(
        &self,
        agent_id: Uuid,
        object_owner: Uuid,
        _object_group: Uuid,
        object_perms: PermissionMask,
    ) -> bool {
        if let Some(v) = self.bypass_check() {
            return v;
        }
        if let Some(v) = self.god_bypass(agent_id) {
            return v;
        }

        // Must have COPY permission.
        if !object_perms.contains(PermissionMask::COPY) {
            return false;
        }

        // Owner can always copy if the bit is set.
        if agent_id == object_owner {
            return true;
        }

        // Non-owner also needs TRANSFER to take a copy.
        if !object_perms.contains(PermissionMask::TRANSFER) {
            return false;
        }

        // Administrator.
        if self.is_administrator(agent_id) {
            return true;
        }

        false
    }

    /// Can the agent transfer this object?
    ///
    /// Port of `CanSellObject` / `CanTakeObject` transfer checks.
    pub fn can_transfer_object(
        &self,
        agent_id: Uuid,
        object_owner: Uuid,
        object_perms: PermissionMask,
    ) -> bool {
        if let Some(v) = self.bypass_check() {
            return v;
        }
        if let Some(v) = self.god_bypass(agent_id) {
            return v;
        }

        // TRANSFER bit must be set.
        if !object_perms.contains(PermissionMask::TRANSFER) {
            return false;
        }

        // Owner or admin.
        if agent_id == object_owner || self.is_administrator(agent_id) {
            return true;
        }

        false
    }

    /// Can the agent take (derez to inventory) this object?
    ///
    /// Port of `CanTakeObject`. Combines owner check + transfer requirement
    /// for non-owners.
    pub fn can_take_object(&self, agent_id: Uuid, object_owner: Uuid, object_group: Uuid) -> bool {
        if let Some(v) = self.bypass_check() {
            return v;
        }
        if let Some(v) = self.god_bypass(agent_id) {
            return v;
        }

        // Owner can always take.
        if agent_id == object_owner {
            return true;
        }

        // Administrator can take.
        if self.is_administrator(agent_id) {
            return true;
        }

        // Group-owned objects: group members with ObjectManipulate power.
        if !object_group.is_nil() && object_owner == object_group {
            // Simplified: in full implementation checks group powers.
        }

        false
    }

    /// Can the agent return objects on the given parcel?
    ///
    /// Port of `CanReturnObjects`.
    pub fn can_return_object(
        &self,
        agent_id: Uuid,
        object_owner: Uuid,
        _object_group: Uuid,
        parcel: Option<&LandData>,
    ) -> bool {
        if let Some(v) = self.bypass_check() {
            return v;
        }
        if let Some(v) = self.god_bypass(agent_id) {
            return v;
        }

        // Owner can return their own objects.
        if agent_id == object_owner {
            return true;
        }

        // Estate manager can return.
        if self.is_estate_manager(agent_id) {
            return true;
        }

        // Administrator can return.
        if self.is_administrator(agent_id) {
            return true;
        }

        // Parcel owner can return objects on their parcel.
        if let Some(parcel) = parcel
            && agent_id == parcel.owner_id
        {
            return true;
        }

        false
    }

    // -----------------------------------------------------------------------
    // Script permissions
    // -----------------------------------------------------------------------

    /// Can scripts run on this parcel for this object owner?
    ///
    /// Port of `CanRunScript`.
    pub fn can_run_script(
        &self,
        agent_id: Uuid,
        object_owner: Uuid,
        parcel: Option<&LandData>,
    ) -> bool {
        if let Some(v) = self.bypass_check() {
            return v;
        }
        if let Some(v) = self.god_bypass(agent_id) {
            return v;
        }

        let parcel = match parcel {
            Some(p) => p,
            None => return false,
        };

        let flags = ParcelFlags::from_bits_truncate(parcel.flags);

        // AllowOtherScripts means any script can run.
        if flags.contains(ParcelFlags::ALLOW_OTHER_SCRIPTS) {
            return true;
        }

        // Object owned by parcel owner can always run scripts.
        if object_owner == parcel.owner_id {
            return true;
        }

        // AllowGroupScripts + matching group.
        if flags.contains(ParcelFlags::ALLOW_GROUP_SCRIPTS) && !parcel.group_id.is_nil() {
            // Simplified: in the full implementation this checks
            // `part.GroupID == parcel.GroupID`. Here we compare the
            // agent to the parcel group as a proxy.
            if agent_id == parcel.group_id {
                return true;
            }
        }

        // Estate permission fallback (estate manager or admin).
        self.generic_estate_permission(object_owner)
    }

    /// Can the agent compile scripts?
    ///
    /// Port of `CanCompileScript`.
    pub fn can_compile_script(&self, agent_id: Uuid) -> bool {
        // In the full implementation this checks per-language grant lists.
        // Here we check if the user is admin or script permission allows all.
        if self.config.allowed_script_creators == UserSet::Administrators {
            return self.is_administrator(agent_id);
        }
        true
    }

    // -----------------------------------------------------------------------
    // World permissions
    // -----------------------------------------------------------------------

    /// Can the agent terraform (edit terrain)?
    ///
    /// Port of `CanTerraformLand`.
    pub fn can_edit_terrain(&self, agent_id: Uuid) -> bool {
        if let Some(v) = self.bypass_check() {
            return v;
        }
        if let Some(v) = self.god_bypass(agent_id) {
            return v;
        }

        // Estate permission (estate manager or administrator).
        self.generic_estate_permission(agent_id)
    }

    /// Can the agent edit parcel properties?
    ///
    /// Port of `CanEditParcelProperties`.
    pub fn can_edit_parcel(&self, agent_id: Uuid, parcel_owner: Uuid) -> bool {
        if let Some(v) = self.bypass_check() {
            return v;
        }
        if let Some(v) = self.god_bypass(agent_id) {
            return v;
        }

        // Parcel owner can edit.
        if agent_id == parcel_owner {
            return true;
        }

        // Administrator can edit.
        if self.is_administrator(agent_id) {
            return true;
        }

        false
    }

    /// Can the agent issue estate commands?
    ///
    /// Port of `CanIssueEstateCommand`.
    pub fn can_edit_estate(&self, agent_id: Uuid) -> bool {
        if let Some(v) = self.bypass_check() {
            return v;
        }
        if let Some(v) = self.god_bypass(agent_id) {
            return v;
        }

        // Estate owner or administrator.
        if agent_id == self.estate_owner {
            return true;
        }

        if self.is_administrator(agent_id) {
            return true;
        }

        false
    }
}
