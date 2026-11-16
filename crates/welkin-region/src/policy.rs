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

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use uuid::Uuid;
use welkin_caps::lru_cache::LruCache;
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

// --- In-process group-name lookup cache -----------------------------------

/// Capacity of the region-global group-name cache.
pub const GROUP_NAME_CACHE_DEFAULT_CAPACITY: usize = 1024;

/// In-process cache type for group id -> group display name.
///
/// Backed by [`welkin_caps::lru_cache::LruCache`] so repeated
/// `UUIDGroupNameRequest` handling does not re-query the groups connector/store
/// for ids already resolved.
pub type GroupNameCache = LruCache<Uuid, String>;

/// Construct a fresh group-name cache with the default capacity.
pub fn new_group_name_cache() -> Arc<GroupNameCache> {
    Arc::new(GroupNameCache::new(GROUP_NAME_CACHE_DEFAULT_CAPACITY))
}

/// Construct a fresh group-name cache with an explicit capacity.
pub fn new_group_name_cache_with_capacity(capacity: usize) -> Arc<GroupNameCache> {
    Arc::new(GroupNameCache::new(capacity))
}

/// Region-global, lazily initialised group-name cache.
///
/// Exposed alongside the module registry so handlers can consult it before
/// issuing connector calls.
pub fn group_name_cache() -> &'static Arc<GroupNameCache> {
    static CACHE: OnceLock<Arc<GroupNameCache>> = OnceLock::new();
    CACHE.get_or_init(new_group_name_cache)
}

/// Alias matching the "registry at region" phrasing used by callers.
pub fn region_group_name_cache() -> &'static Arc<GroupNameCache> {
    group_name_cache()
}

/// Alias for tests/callers expecting a direct accessor.
pub fn group_name_lookup_cache() -> &'static Arc<GroupNameCache> {
    group_name_cache()
}

/// Resolve a single group name, consulting `cache` first.
///
/// Returns `"Unknown"` for a nil id or a missing lookup; callers may override.
pub async fn resolve_group_name_cached(
    cache: &GroupNameCache,
    store: &welkin_data::groups::GroupsStore,
    id: Uuid,
) -> String {
    if id.is_nil() {
        return "Unknown".to_string();
    }
    if let Some(name) = cache.get(&id) {
        return name;
    }
    let names = welkin_services::groups::resolve_group_names(store, &[id]).await;
    let name = names.get(&id).cloned().unwrap_or_else(|| "Unknown".to_string());
    cache.put(id, name.clone());
    name
}

/// Resolve a batch of group names, consulting `cache` first and filling misses.
pub async fn resolve_group_names_cached(
    cache: &GroupNameCache,
    store: &welkin_data::groups::GroupsStore,
    ids: &[Uuid],
) -> HashMap<Uuid, String> {
    let mut out = HashMap::new();
    let mut misses = Vec::new();
    for id in ids {
        if id.is_nil() {
            out.insert(*id, "Unknown".to_string());
            continue;
        }
        if let Some(name) = cache.get(id) {
            out.insert(*id, name);
        } else {
            misses.push(*id);
        }
    }
    if !misses.is_empty() {
        let resolved = welkin_services::groups::resolve_group_names(store, &misses).await;
        for id in &misses {
            let name = resolved.get(id).cloned().unwrap_or_else(|| "Unknown".to_string());
            cache.put(*id, name.clone());
            out.insert(*id, name);
        }
    }
    out
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

    #[test]
    fn group_name_cache_put_get() {
        let cache = new_group_name_cache_with_capacity(4);
        let id = Uuid::new_v4();
        cache.put(id, "Test Group".to_string());
        assert_eq!(cache.get(&id), Some("Test Group".to_string()));
        assert!(group_name_cache().len() <= group_name_cache().capacity());
    }
}