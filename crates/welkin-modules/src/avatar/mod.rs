// SPDX-License-Identifier: GPL-3.0-or-later

//! Avatar modules: appearance/baked textures, friends, lure, gods, dialog,
//! combat, and gestures.
//!
//! Implements the Rust equivalents of the C# avatar region modules from
//! `OpenSim/Region/CoreModules/Avatar/`.

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

    /// Stub: signal appearance to the viewer pipeline. Returns `true` on success.
    pub fn send_appearance(&self, _agent_id: Uuid) -> bool {
        // In a full implementation this would serialize the appearance into
        // viewer packets and push them to the LLUDP layer.
        true
    }

    /// Return the baked texture faces for an agent.
    pub fn get_baked_texture_faces(&self, agent_id: Uuid) -> HashMap<BakeType, Uuid> {
        self.bake_cache.get(&agent_id).cloned().unwrap_or_default()
    }

    /// Persist baked textures to the asset service (stub).
    pub fn save_baked_textures(&mut self, agent_id: Uuid, textures: HashMap<BakeType, Uuid>) -> bool {
        self.bake_cache.insert(agent_id, textures);
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

    /// Request the viewer to rebake textures (stub).
    pub fn request_rebake(&self, _agent_id: Uuid) -> bool {
        true
    }

    /// Update the hover-Z preference for an agent.
    pub fn set_preferences_hover_z(&mut self, agent_id: Uuid, hover_z: f32) {
        if let Some(appearance) = self.appearances.get_mut(&agent_id) {
            appearance.hover_z = hover_z;
        }
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

    /// Mark an agent as online and notify friends (stub notification).
    pub fn status_notify(&mut self, agent_id: Uuid, is_online: bool) {
        if is_online {
            self.online.insert(agent_id);
        } else {
            self.online.remove(&agent_id);
        }
    }

    /// Check whether a friend is currently online in this region.
    pub fn is_friend_online(&self, agent_id: Uuid) -> bool {
        self.online.contains(&agent_id)
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
        self.god_levels.get(&agent_id).copied().unwrap_or(GodLevel::Not)
    }

    /// Kick a user from the region (stub — logs intent, would send kick via LLUDP).
    pub fn kick_user(&self, _agent_id: Uuid, _reason: &str) {
        // Stub: in a full implementation this sends a KickUser packet.
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
        // Stub: would send an AgentAlertMessage packet.
    }

    /// Send an alert message with a modal flag.
    pub fn send_alert_to_user(&self, _agent_id: Uuid, _message: &str, _modal: bool) {
        // Stub: would send a modal/non-modal alert.
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
        // Stub: would send a LoadURL packet.
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
        // Stub: would send a ScriptDialog with a text input box.
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
