// SPDX-License-Identifier: LGPL-3.0-or-later

//! Avatar modules: appearance/baked textures, friends, lure, gods, dialog,
//! combat, and gestures.
//!
//! Implements the Rust equivalents of the C# avatar region modules from
//! `OpenSim/Region/CoreModules/Avatar/`.

use crate::egress::EgressEvent;
use std::collections::HashMap;

use uuid::Uuid;

use crate::module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
use crate::registry::SceneRef;

// ===========================================================================
// WearableType
// ===========================================================================

/// Wearable-type identifiers matching the Second Life / OpenSimulator protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum WearableType {
    Shape = 0,
    Skin = 1,
    Hair = 2,
    Eyes = 3,
    Shirt = 4,
    Pants = 5,
    Shoes = 6,
    Socks = 7,
    Jacket = 8,
    Gloves = 9,
    Undershirt = 10,
    Underpants = 11,
    Skirt = 12,
    Alpha = 13,
    Tattoo = 14,
    Physics = 15,
    Universal = 16,
}

impl WearableType {
    /// All variants in order, for enumeration.
    pub const ALL: [WearableType; 17] = [
        WearableType::Shape,
        WearableType::Skin,
        WearableType::Hair,
        WearableType::Eyes,
        WearableType::Shirt,
        WearableType::Pants,
        WearableType::Shoes,
        WearableType::Socks,
        WearableType::Jacket,
        WearableType::Gloves,
        WearableType::Undershirt,
        WearableType::Underpants,
        WearableType::Skirt,
        WearableType::Alpha,
        WearableType::Tattoo,
        WearableType::Physics,
        WearableType::Universal,
    ];
}

// ===========================================================================
// WearableItem
// ===========================================================================

/// A single wearable worn by an avatar — an inventory item backed by an asset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WearableItem {
    pub item_id: Uuid,
    pub asset_id: Uuid,
}

// ===========================================================================
// BakeType
// ===========================================================================

/// Bake-texture layer identifiers used by the viewer compositing pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum BakeType {
    Head = 0,
    UpperBody = 1,
    LowerBody = 2,
    Eyes = 3,
    Skirt = 4,
    Hair = 5,
    BakedLeftArm = 6,
    BakedLeftLeg = 7,
    BakedAux1 = 8,
    BakedAux2 = 9,
    BakedAux3 = 10,
    NumBakeTypes = 11,
}

impl BakeType {
    /// All variants in order (excluding the count sentinel `NumBakeTypes`).
    pub const ALL: [BakeType; 11] = [
        BakeType::Head,
        BakeType::UpperBody,
        BakeType::LowerBody,
        BakeType::Eyes,
        BakeType::Skirt,
        BakeType::Hair,
        BakeType::BakedLeftArm,
        BakeType::BakedLeftLeg,
        BakeType::BakedAux1,
        BakeType::BakedAux2,
        BakeType::BakedAux3,
    ];
}

// ===========================================================================
// AvatarAppearance
// ===========================================================================

/// Complete avatar appearance data, mirroring C# `AvatarAppearance`.
#[derive(Debug, Clone)]
pub struct AvatarAppearance {
    /// Monotonically increasing version counter.
    pub serial: u32,
    /// Visual-parameter byte array (shape sliders, etc.).
    pub visual_params: Vec<u8>,
    /// Texture-entry byte array (baked textures on each face).
    pub texture_entry: Vec<u8>,
    /// Wearables per slot; each slot can hold multiple layers.
    pub wearables: HashMap<WearableType, Vec<WearableItem>>,
    /// Computed avatar height in metres.
    pub height: f32,
    /// Hover-Z preference offset.
    pub hover_z: f32,
}

impl Default for AvatarAppearance {
    fn default() -> Self {
        Self {
            serial: 0,
            visual_params: Vec::new(),
            texture_entry: Vec::new(),
            wearables: HashMap::new(),
            height: 1.8,
            hover_z: 0.0,
        }
    }
}

// ===========================================================================
// AvatarFactoryModule (NonShared)
// ===========================================================================

/// Manages avatar appearance and baked textures for a single region.
///
/// Rust equivalent of C# `AvatarFactoryModule`.
pub struct AvatarFactoryModule {
    /// Per-agent appearance data.
    appearances: HashMap<Uuid, AvatarAppearance>,
    /// Baked texture cache: agent -> (bake type -> texture uuid).
    bake_cache: HashMap<Uuid, HashMap<BakeType, Uuid>>,
    scene: Option<SceneRef>,
}

impl AvatarFactoryModule {
    pub fn new() -> Self {
        Self {
            appearances: HashMap::new(),
            bake_cache: HashMap::new(),
            scene: None,
        }
    }

    /// Set the full appearance for an agent, incrementing the serial number.
    pub fn set_appearance(&mut self, agent_id: Uuid, mut appearance: AvatarAppearance) {
        appearance.serial = self
            .appearances
            .get(&agent_id)
            .map(|a| a.serial + 1)
            .unwrap_or(1);
        self.appearances.insert(agent_id, appearance);
    }

    /// Retrieve the current appearance (if any).
    pub fn get_appearance(&self, agent_id: Uuid) -> Option<&AvatarAppearance> {
        self.appearances.get(&agent_id)
    }

    pub fn send_appearance(&self, agent_id: Uuid) -> bool {
        if !self.appearances.contains_key(&agent_id) {
            return false;
        }

        if let Some(scene) = &self.scene {
            scene
                .egress
                .send(crate::egress::EgressEvent::AppearanceUpdate { agent: agent_id })
        } else {
            false
        }
    }

    /// Return the baked texture faces for an agent.
    pub fn get_baked_texture_faces(&self, agent_id: Uuid) -> HashMap<BakeType, Uuid> {
        self.bake_cache.get(&agent_id).cloned().unwrap_or_default()
    }

    /// Persist baked avatar textures into the local bake cache.
    ///
    /// Mirrors the OpenSim checks that can be performed in this module: a tracked
    /// region requires an agent presence, and bakes that are nil or use the default
    /// avatar texture are skipped. Remaining bakes are cached and `true` is
    /// returned only if at least one bake survives validation.
    pub fn save_baked_textures(
        &mut self,
        agent_id: Uuid,
        textures: HashMap<BakeType, Uuid>,
    ) -> bool {
        if let Some(scene) = &self.scene {
            let has_presence =
                scene.egress.is_connected(agent_id) || self.appearances.contains_key(&agent_id);
            if !has_presence {
                return false;
            }
        }

        let default_avatar_texture =
            Uuid::parse_str("c228d1cf-4b5d-4ba8-84f4-899a0796acbb").unwrap_or_else(|_| Uuid::nil());

        let baked: HashMap<BakeType, Uuid> = textures
            .into_iter()
            .filter(|(_, texture_id)| {
                !texture_id.is_nil() && *texture_id != default_avatar_texture
            })
            .collect();

        if baked.is_empty() {
            return false;
        }

        self.bake_cache.insert(agent_id, baked);
        true
    }

    /// Validate that all baked textures are present in the cache.
    pub fn validate_baked_texture_cache(&self, agent_id: Uuid) -> bool {
        if let Some(cache) = self.bake_cache.get(&agent_id) {
            !cache.is_empty() && cache.values().all(|id| !id.is_nil())
        } else {
            false
        }
    }

    pub fn request_rebake(&self, agent_id: Uuid) -> bool {
        if let Some(scene) = &self.scene {
            scene
                .egress
                .send(crate::egress::EgressEvent::RebakeRequest { agent: agent_id })
        } else {
            false
        }
    }

    /// Update the hover-Z preference for an agent.
    pub fn set_preferences_hover_z(&mut self, agent_id: Uuid, hover_z: f32) {
        if let Some(appearance) = self.appearances.get_mut(&agent_id) {
            appearance.hover_z = hover_z;
        }
    }
}

impl Default for AvatarFactoryModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for AvatarFactoryModule {
    fn name(&self) -> &str {
        "AvatarFactoryModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::NonShared
    }

    fn initialise(&mut self, _config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.scene = Some(scene.clone());
        Ok(())
    }

    fn remove_region(&mut self, _scene: &SceneRef) -> Result<(), ModuleError> {
        self.appearances.clear();
        self.bake_cache.clear();
        self.scene = None;
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.appearances.clear();
        self.bake_cache.clear();
        Ok(())
    }
}

// ===========================================================================
// BakedTexturesModule (Shared)
// ===========================================================================

/// Shared cache of baked textures across regions.
///
/// Rust equivalent of the baked texture caching portion of C#
/// `AvatarFactoryModule`, separated as a shared service.
pub struct BakedTexturesModule {
    /// (agent_id, bake_type) -> texture_id.
    cache: HashMap<(Uuid, BakeType), Uuid>,
}

impl BakedTexturesModule {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    /// Return cached items for an agent.
    pub fn get_cached_items(&self, agent_id: Uuid) -> Vec<(BakeType, Uuid)> {
        self.cache
            .iter()
            .filter_map(|(&(aid, bt), &tid)| {
                if aid == agent_id {
                    Some((bt, tid))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Store a baked texture into the cache.
    pub fn store_baked_texture(&mut self, agent_id: Uuid, bake_type: BakeType, texture_id: Uuid) {
        self.cache.insert((agent_id, bake_type), texture_id);
    }

    /// Clear all cached textures for an agent.
    pub fn clear_cache(&mut self, agent_id: Uuid) {
        self.cache.retain(|&(aid, _), _| aid != agent_id);
    }
}

impl Default for BakedTexturesModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for BakedTexturesModule {
    fn name(&self) -> &str {
        "BakedTexturesModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::Shared
    }

    fn initialise(&mut self, _config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        Ok(())
    }

    fn add_region(&mut self, _scene: &SceneRef) -> Result<(), ModuleError> {
        Ok(())
    }

    fn remove_region(&mut self, _scene: &SceneRef) -> Result<(), ModuleError> {
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.cache.clear();
        Ok(())
    }
}

// ===========================================================================
// FriendPerms (bitflags)
// ===========================================================================

bitflags::bitflags! {
    /// Friend permission flags, matching C# `FriendRights`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct FriendPerms: u32 {
        const NONE              = 0;
        const CAN_SEE_ONLINE    = 1;
        const CAN_SEE_ON_MAP    = 2;
        const CAN_MODIFY_OBJECTS = 4;
    }
}

// ===========================================================================
// FriendInfo
// ===========================================================================

/// Describes a friendship between two agents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FriendInfo {
    pub friend_id: Uuid,
    pub my_flags: FriendPerms,
    pub their_flags: FriendPerms,
}

// ===========================================================================
// FriendsModule (NonShared)
// ===========================================================================

/// Manages the friends list and online-status notifications for a region.
///
/// Rust equivalent of C# `FriendsModule`.
pub struct FriendsModule {
    /// agent_id -> list of friends.
    friends: HashMap<Uuid, Vec<FriendInfo>>,
    /// Agents currently known to be online (in this region).
    online: std::collections::HashSet<Uuid>,
    scene: Option<SceneRef>,
}

impl FriendsModule {
    pub fn new() -> Self {
        Self {
            friends: HashMap::new(),
            online: std::collections::HashSet::new(),
            scene: None,
        }
    }

    /// Add a bidirectional friendship.
    pub fn add_friend(&mut self, agent_id: Uuid, friend_id: Uuid) {
        // Add to agent's list.
        let entry = FriendInfo {
            friend_id,
            my_flags: FriendPerms::CAN_SEE_ONLINE,
            their_flags: FriendPerms::CAN_SEE_ONLINE,
        };
        self.friends.entry(agent_id).or_default().push(entry);

        // Add reverse entry.
        let reverse = FriendInfo {
            friend_id: agent_id,
            my_flags: FriendPerms::CAN_SEE_ONLINE,
            their_flags: FriendPerms::CAN_SEE_ONLINE,
        };
        self.friends.entry(friend_id).or_default().push(reverse);
    }

    /// Remove a friendship in both directions.
    pub fn remove_friend(&mut self, agent_id: Uuid, friend_id: Uuid) {
        if let Some(list) = self.friends.get_mut(&agent_id) {
            list.retain(|f| f.friend_id != friend_id);
        }
        if let Some(list) = self.friends.get_mut(&friend_id) {
            list.retain(|f| f.friend_id != agent_id);
        }
    }

    /// Return the friend list for an agent.
    pub fn get_friends(&self, agent_id: Uuid) -> Vec<FriendInfo> {
        self.friends.get(&agent_id).cloned().unwrap_or_default()
    }

    /// Grant or update permission flags that `agent_id` gives to `friend_id`.
    pub fn grant_rights(&mut self, agent_id: Uuid, friend_id: Uuid, rights: FriendPerms) {
        if let Some(list) = self.friends.get_mut(&agent_id) {
            for f in list.iter_mut() {
                if f.friend_id == friend_id {
                    f.my_flags = rights;
                }
            }
        }
        // Update the reverse direction's their_flags.
        if let Some(list) = self.friends.get_mut(&friend_id) {
            for f in list.iter_mut() {
                if f.friend_id == agent_id {
                    f.their_flags = rights;
                }
            }
        }
    }

    /// Mark an agent as online/offline and notify each listed friend of the status change.
    /// Notifications are delivered through the region egress; disconnected friends simply
    /// have their event dropped by the connected-egress layer.
    pub fn status_notify(&mut self, agent_id: Uuid, is_online: bool) {
        if is_online {
            self.online.insert(agent_id);
        } else {
            self.online.remove(&agent_id);
        }

        let friends: Vec<Uuid> = self
            .friends
            .get(&agent_id)
            .map(|list| list.iter().map(|friend| friend.friend_id).collect())
            .unwrap_or_default();

        if let Some(scene) = &self.scene {
            for friend in friends {
                let _ = scene.egress.send(EgressEvent::FriendOnline {
                    agent: friend,
                    friend: agent_id,
                    online: is_online,
                });
            }
        }
    }

    /// Check whether a friend is currently online in this region.
    pub fn is_friend_online(&self, agent_id: Uuid) -> bool {
        self.online.contains(&agent_id)
    }
}

impl Default for FriendsModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for FriendsModule {
    fn name(&self) -> &str {
        "FriendsModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::NonShared
    }

    fn initialise(&mut self, _config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.scene = Some(scene.clone());
        Ok(())
    }

    fn remove_region(&mut self, _scene: &SceneRef) -> Result<(), ModuleError> {
        self.friends.clear();
        self.online.clear();
        self.scene = None;
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.friends.clear();
        self.online.clear();
        Ok(())
    }
}

// ===========================================================================
// LureModule (NonShared)
// ===========================================================================

/// Handles teleport lure invitations.
///
/// Rust equivalent of C# `LureModule`.
pub struct LureModule {
    /// Pending lure requests: lure_id -> (from, to, message).
    pending: HashMap<Uuid, (Uuid, Uuid, String)>,
    scene: Option<SceneRef>,
}

impl LureModule {
    pub fn new() -> Self {
        Self {
            pending: HashMap::new(),
            scene: None,
        }
    }

    /// Send a teleport lure from one agent to another.
    /// Returns the generated lure ID.
    pub fn send_lure(&mut self, from: Uuid, to: Uuid, message: &str) -> Uuid {
        let lure_id = Uuid::new_v4();
        self.pending
            .insert(lure_id, (from, to, message.to_string()));
        lure_id
    }

    /// Process a lure acceptance: the target agent requests teleport.
    /// Returns `true` if the lure was found, matched, and consumed.
    pub fn teleport_lure_request(&mut self, from: Uuid, to: Uuid, lure_id: Uuid) -> bool {
        let matched = self
            .pending
            .get(&lure_id)
            .map(|(sf, st, _)| *sf == from && *st == to)
            .unwrap_or(false);
        if matched {
            self.pending.remove(&lure_id);
        }
        matched
    }

    /// Number of pending lure requests.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
}

impl Default for LureModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for LureModule {
    fn name(&self) -> &str {
        "LureModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::NonShared
    }

    fn initialise(&mut self, _config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.scene = Some(scene.clone());
        Ok(())
    }

    fn remove_region(&mut self, _scene: &SceneRef) -> Result<(), ModuleError> {
        self.pending.clear();
        self.scene = None;
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.pending.clear();
        Ok(())
    }
}

// ===========================================================================
// GodLevel
// ===========================================================================

/// God-mode levels matching the C# `GodsModule` constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum GodLevel {
    Not = 0,
    God = 200,
    FullGod = 255,
}

// ===========================================================================
// GodsModule (NonShared)
// ===========================================================================

/// Manages god-mode requests and administrative kicks.
///
/// Rust equivalent of C# `GodsModule`.
pub struct GodsModule {
    /// Per-agent god level.
    god_levels: HashMap<Uuid, GodLevel>,
    /// Whether to show a dialog on region exit.
    pub dialog_on_exit: bool,
    scene: Option<SceneRef>,
}

impl GodsModule {
    pub fn new() -> Self {
        Self {
            god_levels: HashMap::new(),
            dialog_on_exit: true,
            scene: None,
        }
    }

    /// Process a god-powers request. In a full implementation this would
    /// verify the agent's grid-level permissions; here we grant if the
    /// requested level is valid.
    pub fn request_god_powers(&mut self, agent_id: Uuid, level: GodLevel) {
        self.god_levels.insert(agent_id, level);
    }

    /// Get the current god level for an agent.
    pub fn get_god_level(&self, agent_id: Uuid) -> GodLevel {
        self.god_levels
            .get(&agent_id)
            .copied()
            .unwrap_or(GodLevel::Not)
    }

    /// Kick a user from the region.
    pub fn kick_user(&self, agent_id: Uuid, reason: &str) {
        if let Some(scene) = &self.scene {
            let event = EgressEvent::Kick {
                agent: agent_id,
                reason: reason.to_string(),
            };
            scene.egress.send(event);
        }
        tracing::info!(agent = %agent_id, reason = %reason, "kicking user from region");
    }
}

impl Default for GodsModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for GodsModule {
    fn name(&self) -> &str {
        "GodsModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::NonShared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        self.dialog_on_exit = config.get_bool("GodsModule", "DialogOnExit", true);
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.scene = Some(scene.clone());
        Ok(())
    }

    fn remove_region(&mut self, _scene: &SceneRef) -> Result<(), ModuleError> {
        self.god_levels.clear();
        self.scene = None;
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.god_levels.clear();
        Ok(())
    }
}

// ===========================================================================
// DialogModule (NonShared)
// ===========================================================================

/// Sends alert dialogs, URL dialogs, and text boxes to viewer clients.
///
/// Rust equivalent of C# `DialogModule`.
pub struct DialogModule {
    scene: Option<SceneRef>,
}

impl DialogModule {
    pub fn new() -> Self {
        Self { scene: None }
    }

    /// Send a simple alert message to a single agent.
    pub fn send_alert(&self, _agent_id: Uuid, _message: &str) {
        if let Some(scene) = &self.scene {
            let message = _message.chars().take(1024).collect::<String>();
            let _ = scene.egress.send(crate::egress::EgressEvent::Alert {
                agent: _agent_id,
                message,
                modal: false,
            });
        }
    }

    /// Send an alert message with a modal flag.
    pub fn send_alert_to_user(&self, _agent_id: Uuid, _message: &str, _modal: bool) {
        if let Some(scene) = &self.scene {
            let message = _message.chars().take(1024).collect::<String>();
            let _ = scene.egress.send(crate::egress::EgressEvent::Alert {
                agent: _agent_id,
                message,
                modal: _modal,
            });
        }
    }

    /// Send a URL notification dialog.
    pub fn send_url_to_user(
        &self,
        _agent_id: Uuid,
        _object_name: &str,
        _object_id: Uuid,
        _owner_id: Uuid,
        _group_owned: bool,
        _message: &str,
        _url: &str,
    ) {
        if let Some(scene) = &self.scene {
            let message = _message.chars().take(1024).collect::<String>();
            let _ = scene.egress.send(crate::egress::EgressEvent::LoadUrl {
                agent: _agent_id,
                object_name: _object_name.to_string(),
                object_id: _object_id,
                owner_id: _owner_id,
                group_owned: _group_owned,
                message,
                url: _url.to_string(),
            });
        }
    }

    /// Send a text-box input dialog.
    pub fn send_text_box_to_user(
        &self,
        _agent_id: Uuid,
        _message: &str,
        _chat_channel: i32,
        _object_name: &str,
        _object_id: Uuid,
        _owner_id: Uuid,
    ) {
        if let Some(scene) = &self.scene {
            let _ = scene.egress.send(crate::egress::EgressEvent::TextBox {
                agent: _agent_id,
                message: _message.to_string(),
                chat_channel: _chat_channel,
                object_name: _object_name.to_string(),
                object_id: _object_id,
                owner_id: _owner_id,
            });
        }
    }
}

impl Default for DialogModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for DialogModule {
    fn name(&self) -> &str {
        "DialogModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::NonShared
    }

    fn initialise(&mut self, _config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.scene = Some(scene.clone());
        Ok(())
    }

    fn remove_region(&mut self, _scene: &SceneRef) -> Result<(), ModuleError> {
        self.scene = None;
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

// ===========================================================================
// CombatModule (NonShared)
// ===========================================================================

/// Simple avatar combat / health tracking.
///
/// Rust equivalent of C# `CombatModule`.
pub struct CombatModule {
    /// Whether damage is enabled in this region.
    pub damage_enabled: bool,
    /// Maximum damage per hit.
    pub max_damage: f32,
    /// Per-agent accumulated damage (health starts at 100.0).
    health: HashMap<Uuid, f32>,
    scene: Option<SceneRef>,
}

impl CombatModule {
    pub fn new() -> Self {
        Self {
            damage_enabled: false,
            max_damage: 100.0,
            health: HashMap::new(),
            scene: None,
        }
    }

    /// Apply damage to an agent. Returns the remaining health.
    pub fn apply_damage(&mut self, agent_id: Uuid, damage: f32) -> f32 {
        if !self.damage_enabled {
            return 100.0;
        }
        let clamped = damage.min(self.max_damage).max(0.0);
        let hp = self.health.entry(agent_id).or_insert(100.0);
        *hp = (*hp - clamped).max(0.0);
        *hp
    }

    /// Called when an agent is killed (health reached 0). Returns the killer ID.
    pub fn killed(&mut self, agent_id: Uuid, killer_id: Uuid) -> Uuid {
        // Reset health to 100 (respawn).
        self.health.insert(agent_id, 100.0);
        killer_id
    }

    /// Get the current health for an agent.
    pub fn get_health(&self, agent_id: Uuid) -> f32 {
        self.health.get(&agent_id).copied().unwrap_or(100.0)
    }
}

impl Default for CombatModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for CombatModule {
    fn name(&self) -> &str {
        "CombatModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::NonShared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        self.damage_enabled = config.get_bool("CombatModule", "DamageEnabled", false);
        self.max_damage = config.get_float("CombatModule", "MaxDamage", 100.0) as f32;
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.scene = Some(scene.clone());
        Ok(())
    }

    fn remove_region(&mut self, _scene: &SceneRef) -> Result<(), ModuleError> {
        self.health.clear();
        self.scene = None;
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.health.clear();
        Ok(())
    }
}

// ===========================================================================
// GesturesModule (NonShared)
// ===========================================================================

/// Tracks active gestures per agent.
///
/// Rust equivalent of C# `GesturesModule`.
pub struct GesturesModule {
    /// agent_id -> set of (asset_id, gesture_id).
    active: HashMap<Uuid, Vec<(Uuid, Uuid)>>,
    scene: Option<SceneRef>,
}

impl GesturesModule {
    pub fn new() -> Self {
        Self {
            active: HashMap::new(),
            scene: None,
        }
    }

    /// Activate a gesture for an agent.
    pub fn activate_gesture(&mut self, agent_id: Uuid, asset_id: Uuid, gesture_id: Uuid) {
        let list = self.active.entry(agent_id).or_default();
        // Don't add duplicates.
        if !list.iter().any(|&(_, gid)| gid == gesture_id) {
            list.push((asset_id, gesture_id));
        }
    }

    /// Deactivate a gesture for an agent. Returns `true` if it was active.
    pub fn deactivate_gesture(&mut self, agent_id: Uuid, gesture_id: Uuid) -> bool {
        if let Some(list) = self.active.get_mut(&agent_id) {
            let before = list.len();
            list.retain(|&(_, gid)| gid != gesture_id);
            list.len() < before
        } else {
            false
        }
    }

    /// Return the active gestures for an agent.
    pub fn get_active_gestures(&self, agent_id: Uuid) -> Vec<(Uuid, Uuid)> {
        self.active.get(&agent_id).cloned().unwrap_or_default()
    }
}

impl Default for GesturesModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for GesturesModule {
    fn name(&self) -> &str {
        "GesturesModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::NonShared
    }

    fn initialise(&mut self, _config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.scene = Some(scene.clone());
        Ok(())
    }

    fn remove_region(&mut self, _scene: &SceneRef) -> Result<(), ModuleError> {
        self.active.clear();
        self.scene = None;
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.active.clear();
        Ok(())
    }
}

#[cfg(test)]
mod stub_tests_avfac {
    use super::*;
    #[allow(unused_imports)]
    use crate::module::RegionModule;
    #[test]
    fn send_appearance_without_stored_appearance_is_false() {
        let agent = Uuid::new_v4();
        let mut module = AvatarFactoryModule {
            appearances: std::collections::HashMap::new(),
            bake_cache: std::collections::HashMap::new(),
            scene: None,
        };

        let mut scene = crate::registry::SceneRef::new(Uuid::new_v4(), "r".to_string());
        let rec = std::sync::Arc::new(crate::egress::RecordingEgress::new());
        scene.egress = rec.clone();

        use crate::module::RegionModule;
        let _ = module.add_region(&scene);

        rec.connect(agent);
        assert!(!module.send_appearance(agent));
        assert!(rec.events().is_empty());
    }

    #[test]
    fn send_appearance_with_stored_appearance_records_event() {
        let agent = Uuid::new_v4();
        let mut module = AvatarFactoryModule {
            appearances: std::collections::HashMap::new(),
            bake_cache: std::collections::HashMap::new(),
            scene: None,
        };

        module.set_appearance(agent, AvatarAppearance::default());

        let mut scene = crate::registry::SceneRef::new(Uuid::new_v4(), "r".to_string());
        let rec = std::sync::Arc::new(crate::egress::RecordingEgress::new());
        scene.egress = rec.clone();

        use crate::module::RegionModule;
        let _ = module.add_region(&scene);

        rec.connect(agent);
        assert!(module.send_appearance(agent));
        assert_eq!(
            rec.events(),
            vec![crate::egress::EgressEvent::AppearanceUpdate { agent }]
        );
    }

    #[test]
    fn request_rebake_records_event() {
        let agent = Uuid::new_v4();
        let mut module = AvatarFactoryModule {
            appearances: std::collections::HashMap::new(),
            bake_cache: std::collections::HashMap::new(),
            scene: None,
        };

        let mut scene = crate::registry::SceneRef::new(Uuid::new_v4(), "r".to_string());
        let rec = std::sync::Arc::new(crate::egress::RecordingEgress::new());
        scene.egress = rec.clone();

        use crate::module::RegionModule;
        let _ = module.add_region(&scene);

        rec.connect(agent);
        assert!(module.request_rebake(agent));
        assert_eq!(
            rec.events(),
            vec![crate::egress::EgressEvent::RebakeRequest { agent }]
        );
    }
}

#[cfg(test)]
mod stub_tests_gods {
    use super::*;
    #[allow(unused_imports)]
    use crate::module::RegionModule;
    #[test]
    fn kick_user_sends_kick_when_connected() {
        use crate::module::RegionModule;

        let agent = uuid::Uuid::new_v4();
        let mut scene = crate::registry::SceneRef::new(uuid::Uuid::new_v4(), "r".to_string());
        let rec = std::sync::Arc::new(crate::egress::RecordingEgress::new());
        scene.egress = rec.clone();

        let mut module = GodsModule {
            god_levels: std::collections::HashMap::new(),
            dialog_on_exit: false,
            scene: None,
        };

        let _ = module.add_region(&scene);
        rec.connect(agent);

        module.kick_user(agent, "because");

        assert_eq!(
            rec.events(),
            vec![crate::egress::EgressEvent::Kick {
                agent,
                reason: "because".to_string()
            }]
        );
    }

    #[test]
    fn kick_user_does_nothing_when_not_connected() {
        use crate::module::RegionModule;

        let agent = uuid::Uuid::new_v4();
        let mut scene = crate::registry::SceneRef::new(uuid::Uuid::new_v4(), "r".to_string());
        let rec = std::sync::Arc::new(crate::egress::RecordingEgress::new());
        scene.egress = rec.clone();

        let mut module = GodsModule {
            god_levels: std::collections::HashMap::new(),
            dialog_on_exit: false,
            scene: None,
        };

        let _ = module.add_region(&scene);

        module.kick_user(agent, "because");

        assert!(rec.events().is_empty());
    }
}

#[cfg(test)]
mod stub_tests_dialog {
    use super::*;
    #[allow(unused_imports)]
    use crate::module::RegionModule;
    #[test]
    fn send_alert_event_arrives() {
        let agent = Uuid::new_v4();
        let mut scene = crate::registry::SceneRef::new(Uuid::new_v4(), "r".to_string());
        let rec = std::sync::Arc::new(crate::egress::RecordingEgress::new());
        scene.egress = rec.clone();
        rec.connect(agent);

        let module = DialogModule { scene: Some(scene) };
        module.send_alert(agent, "hello");

        assert_eq!(
            rec.events(),
            vec![crate::egress::EgressEvent::Alert {
                agent,
                message: "hello".to_string(),
                modal: false,
            }]
        );
    }

    #[test]
    fn send_alert_to_user_event_arrives() {
        let agent = Uuid::new_v4();
        let mut scene = crate::registry::SceneRef::new(Uuid::new_v4(), "r".to_string());
        let rec = std::sync::Arc::new(crate::egress::RecordingEgress::new());
        scene.egress = rec.clone();
        rec.connect(agent);

        let module = DialogModule { scene: Some(scene) };
        module.send_alert_to_user(agent, "attention", true);

        assert_eq!(
            rec.events(),
            vec![crate::egress::EgressEvent::Alert {
                agent,
                message: "attention".to_string(),
                modal: true,
            }]
        );
    }

    #[test]
    fn send_url_to_user_event_arrives() {
        let agent = Uuid::new_v4();
        let object_id = Uuid::new_v4();
        let owner_id = Uuid::new_v4();
        let mut scene = crate::registry::SceneRef::new(Uuid::new_v4(), "r".to_string());
        let rec = std::sync::Arc::new(crate::egress::RecordingEgress::new());
        scene.egress = rec.clone();
        rec.connect(agent);

        let module = DialogModule { scene: Some(scene) };
        module.send_url_to_user(agent, "object", object_id, owner_id, true, "message", "https://example.com");

        assert_eq!(
            rec.events(),
            vec![crate::egress::EgressEvent::LoadUrl {
                agent,
                object_name: "object".to_string(),
                object_id,
                owner_id,
                group_owned: true,
                message: "message".to_string(),
                url: "https://example.com".to_string(),
            }]
        );
    }

    #[test]
    fn send_text_box_to_user_event_arrives() {
        let agent = Uuid::new_v4();
        let object_id = Uuid::new_v4();
        let owner_id = Uuid::new_v4();
        let mut scene = crate::registry::SceneRef::new(Uuid::new_v4(), "r".to_string());
        let rec = std::sync::Arc::new(crate::egress::RecordingEgress::new());
        scene.egress = rec.clone();
        rec.connect(agent);

        let module = DialogModule { scene: Some(scene) };
        module.send_text_box_to_user(agent, "prompt", 42, "object", object_id, owner_id);

        assert_eq!(
            rec.events(),
            vec![crate::egress::EgressEvent::TextBox {
                agent,
                message: "prompt".to_string(),
                chat_channel: 42,
                object_name: "object".to_string(),
                object_id,
                owner_id,
            }]
        );
    }

    #[test]
    fn send_alert_truncates_to_1024_chars() {
        let agent = Uuid::new_v4();
        let mut scene = crate::registry::SceneRef::new(Uuid::new_v4(), "r".to_string());
        let rec = std::sync::Arc::new(crate::egress::RecordingEgress::new());
        scene.egress = rec.clone();
        rec.connect(agent);

        let module = DialogModule { scene: Some(scene) };
        module.send_alert(agent, &"x".repeat(2000));

        assert_eq!(
            rec.events(),
            vec![crate::egress::EgressEvent::Alert {
                agent,
                message: "x".repeat(1024),
                modal: false,
            }]
        );
    }

    #[test]
    fn send_does_nothing_without_region() {
        let agent = Uuid::new_v4();
        let rec = std::sync::Arc::new(crate::egress::RecordingEgress::new());
        let module = DialogModule { scene: None };
        module.send_alert(agent, "hello");
        module.send_alert_to_user(agent, "hello", true);
        module.send_url_to_user(
            agent,
            "object",
            Uuid::new_v4(),
            Uuid::new_v4(),
            false,
            "hello",
            "https://example.com",
        );
        module.send_text_box_to_user(agent, "hello", 1, "object", Uuid::new_v4(), Uuid::new_v4());
        assert!(rec.events().is_empty());
    }
}

#[cfg(test)]
#[allow(unused_mut, unused_variables)]
mod stub_tests_s_avatarfactorymodule_0 {
    #[allow(unused_imports)]
    use super::*;
    #[allow(unused_imports)]
    use crate::module::RegionModule;
    #[test]
    fn save_baked_textures_caches_valid_bakes() {
        use std::collections::HashMap;

        let agent = Uuid::new_v4();
        let mut module = AvatarFactoryModule {
            appearances: HashMap::new(),
            bake_cache: HashMap::new(),
            scene: None,
        };

        let head = Uuid::new_v4();
        let mut textures = HashMap::new();
        textures.insert(BakeType::Head, head);

        assert!(module.save_baked_textures(agent, textures));
        let cached = module
            .bake_cache
            .get(&agent)
            .expect("baked textures should be cached");
        assert_eq!(cached.get(&BakeType::Head), Some(&head));
    }

    #[test]
    fn save_baked_textures_stores_multiple_bake_types() {
        use std::collections::HashMap;

        let agent = Uuid::new_v4();
        let mut module = AvatarFactoryModule {
            appearances: HashMap::new(),
            bake_cache: HashMap::new(),
            scene: None,
        };

        let head = Uuid::new_v4();
        let upper_body = Uuid::new_v4();
        let mut textures = HashMap::new();
        textures.insert(BakeType::Head, head);
        textures.insert(BakeType::UpperBody, upper_body);

        assert!(module.save_baked_textures(agent, textures));
        let cached = module.bake_cache.get(&agent).expect("cache entry");
        assert_eq!(cached.len(), 2);
        assert_eq!(cached.get(&BakeType::UpperBody), Some(&upper_body));
    }

    #[test]
    fn save_baked_textures_requires_presence_for_tracked_scene() {
        use std::collections::HashMap;

        let agent = Uuid::new_v4();
        let mut scene = crate::registry::SceneRef::new(Uuid::new_v4(), "r".to_string());
        let rec = std::sync::Arc::new(crate::egress::RecordingEgress::new());
        scene.egress = rec.clone();

        let mut module = AvatarFactoryModule {
            appearances: HashMap::new(),
            bake_cache: HashMap::new(),
            scene: None,
        };
        let _ = module.add_region(&scene);

        let mut textures = HashMap::new();
        textures.insert(BakeType::Head, Uuid::new_v4());

        assert!(!module.save_baked_textures(agent, textures.clone()));
        assert!(module.bake_cache.is_empty());

        rec.connect(agent);
        assert!(module.save_baked_textures(agent, textures));
        assert!(module.bake_cache.contains_key(&agent));
    }

    #[test]
    fn save_baked_textures_rejects_nil_default_and_empty_bakes() {
        use std::collections::HashMap;

        let agent = Uuid::new_v4();
        let mut module = AvatarFactoryModule {
            appearances: HashMap::new(),
            bake_cache: HashMap::new(),
            scene: None,
        };

        let default_texture =
            Uuid::parse_str("c228d1cf-4b5d-4ba8-84f4-899a0796acbb").unwrap_or(Uuid::nil());

        let mut only_default = HashMap::new();
        only_default.insert(BakeType::Head, default_texture);
        assert!(!module.save_baked_textures(agent, only_default));

        let mut only_nil = HashMap::new();
        only_nil.insert(BakeType::Head, Uuid::nil());
        assert!(!module.save_baked_textures(agent, only_nil));

        assert!(!module.save_baked_textures(agent, HashMap::new()));
        assert!(module.bake_cache.is_empty());
    }
}

#[cfg(test)]
#[allow(unused_mut, unused_variables)]
mod stub_tests_mod_rs_friendsmodule_0 {
    #[allow(unused_imports)]
    use super::*;
    #[allow(unused_imports)]
    use crate::module::RegionModule;
    #[test]
    fn status_notify_updates_online_state() {
        let mut module = FriendsModule {
            friends: std::collections::HashMap::new(),
            online: std::collections::HashSet::new(),
            scene: None,
        };
        let agent = Uuid::new_v4();

        module.status_notify(agent, true);
        assert!(module.online.contains(&agent));

        module.status_notify(agent, false);
        assert!(!module.online.contains(&agent));
    }

    #[test]
    fn status_notify_without_friends_emit_no_egress_events() {
        let mut scene = crate::registry::SceneRef::new(Uuid::new_v4(), "r".to_string());
        let rec = std::sync::Arc::new(crate::egress::RecordingEgress::new());
        scene.egress = rec.clone();

        let mut module = FriendsModule {
            friends: std::collections::HashMap::new(),
            online: std::collections::HashSet::new(),
            scene: Some(scene),
        };
        let agent = Uuid::new_v4();

        module.status_notify(agent, true);

        assert!(module.online.contains(&agent));
        assert!(rec.take().is_empty());
    }
}
