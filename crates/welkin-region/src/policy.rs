// SPDX-License-Identifier: LGPL-3.0-or-later
#![allow(dead_code)]

//! Region-level policy helper.
//!
//! Bridges `welkin_scene::Scene::set_avatar_frozen` with
//! `welkin_modules::DefaultPermissionsModule`, so a "freeze user" action
//! performed by an authorised administrator actually constrains the target's
//! physics/movement instead of only sending an alert.
//!
//! The helper is sync and pure: it never performs I/O and never panics.

use uuid::Uuid;
use welkin_modules::DefaultPermissionsModule;
use welkin_scene::scene::Scene;

/// Why a freeze/unfreeze request was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FreezeError {
    /// The requester is not authorised to freeze avatars.
    PermissionDenied,
    /// The target avatar is not present in the scene.
    TargetNotFound,
}

/// Outcome of a successful policy evaluation.
pub type FreezeResult = Result<bool, FreezeError>;

/// Is the requester authorised to freeze/unfreeze avatars?
///
/// Considers explicit gods, automatic/implicit gods and estate administrators
/// through `DefaultPermissionsModule`.
pub fn can_freeze(perms: &DefaultPermissionsModule, requester: Uuid) -> bool {
    if requester.is_nil() {
        return false;
    }
    perms.is_god(requester) || perms.is_administrator(requester) || perms.generic_estate_permission(requester)
}

/// Apply a freeze/unfreeze decision through region policy.
///
/// Returns `Ok(true)` when the scene actually changed the avatar's frozen
/// state, `Ok(false)` if the scene reported the avatar unknown (should not
/// happen after the presence check, but surfaced for completeness),
/// `Err(FreezeError::PermissionDenied)` if `requester` is not authorised, and
/// `Err(FreezeError::TargetNotFound)` if the target has no presence in
/// `scene`.
pub fn apply_freeze_policy(
    scene: &Scene,
    perms: &DefaultPermissionsModule,
    requester: Uuid,
    target: Uuid,
    freeze: bool,
) -> FreezeResult {
    if !can_freeze(perms, requester) {
        return Err(FreezeError::PermissionDenied);
    }
    if scene.get_presence(&target).is_none() {
        return Err(FreezeError::TargetNotFound);
    }
    Ok(scene.set_avatar_frozen(target, freeze))
}

// Convenience aliases --------------------------------------------------------
#[allow(dead_code)]
pub fn freeze_avatar(
    scene: &Scene,
    perms: &DefaultPermissionsModule,
    requester: Uuid,
    target: Uuid,
    freeze: bool,
) -> FreezeResult {
    apply_freeze_policy(scene, perms, requester, target, freeze)
}

pub fn freeze_user(
    scene: &Scene,
    perms: &DefaultPermissionsModule,
    requester: Uuid,
    target: Uuid,
    freeze: bool,
) -> FreezeResult {
    apply_freeze_policy(scene, perms, requester, target, freeze)
}

pub fn set_avatar_frozen_with_policy(
    scene: &Scene,
    perms: &DefaultPermissionsModule,
    requester: Uuid,
    target: Uuid,
    freeze: bool,
) -> FreezeResult {
    apply_freeze_policy(scene, perms, requester, target, freeze)
}

pub fn apply_freeze(
    scene: &Scene,
    perms: &DefaultPermissionsModule,
    requester: Uuid,
    target: Uuid,
    freeze: bool,
) -> FreezeResult {
    apply_freeze_policy(scene, perms, requester, target, freeze)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;
    #[test]
    fn permission_denied_for_unknown_requester() {
        let perms = DefaultPermissionsModule::new();
        let requester = Uuid::new_v4();
        let target = Uuid::new_v4();
        // Cannot build Scene without config легко; check permission only.
        assert!(!can_freeze(&perms, requester));
        let _ = (target,);
    }
}