// SPDX-License-Identifier: LGPL-3.0-or-later

//! Estate management module: estate settings, access control, telehub
//! management, and estate-level actions.

use std::collections::HashSet;
use std::sync::mpsc;

use uuid::Uuid;

use crate::module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
use crate::registry::SceneRef;

/// Actions dispatched by estate management methods for the region to execute.
#[derive(Debug, Clone)]
pub enum EstateAction {
    Restart { delay_seconds: u32 },
    Kick { agent_id: Uuid, message: String },
    TeleportHome { agent_id: Uuid },
    RegionMessage { sender: String, message: String },
}

// ---------------------------------------------------------------------------
// EstateSettings
// ---------------------------------------------------------------------------

/// Persistent estate-level configuration, mirroring the C# `EstateSettings`
/// class.
#[derive(Debug, Clone)]
pub struct EstateSettings {
    pub estate_id: u32,
    pub estate_name: String,
    pub estate_owner: Uuid,
    pub abuse_email: String,

    // Access control flags.
    pub deny_anonymous: bool,
    pub deny_identified: bool,
    pub allow_voice: bool,
    pub allow_direct_teleport: bool,
    pub deny_transacted: bool,
    pub deny_age_unverified: bool,
    pub deny_minors: bool,

    // Economy.
    pub price_per_meter: i32,

    // Redirect grid coordinates.
    pub redirect_grid_x: i32,
    pub redirect_grid_y: i32,

    // Sun / time.
    pub use_global_time: bool,
    pub fixed_sun: bool,
    pub sun_position: f64,

    // Covenant.
    pub covenant: Uuid,
    pub covenant_timestamp: i64,

    pub abuse_email_to_estate_owner: bool,

    // Access lists.
    pub estate_managers: HashSet<Uuid>,
    pub estate_users: HashSet<Uuid>,
    pub estate_banned: HashSet<Uuid>,
    pub estate_groups: HashSet<Uuid>,

    // Terrain textures and elevation bounds.
    pub terrain_textures: [Uuid; 4],
    pub elevation_bounds: [(f32, f32); 4],

    pub water_height: f64,
}

impl Default for EstateSettings {
    fn default() -> Self {
        Self {
            estate_id: 0,
            estate_name: String::from("My Estate"),
            estate_owner: Uuid::nil(),
            abuse_email: String::new(),
            deny_anonymous: false,
            deny_identified: false,
            allow_voice: true,
            allow_direct_teleport: true,
            deny_transacted: false,
            deny_age_unverified: false,
            deny_minors: false,
            price_per_meter: 1,
            redirect_grid_x: 0,
            redirect_grid_y: 0,
            use_global_time: true,
            fixed_sun: false,
            sun_position: 0.0,
            covenant: Uuid::nil(),
            covenant_timestamp: 0,
            abuse_email_to_estate_owner: false,
            estate_managers: HashSet::new(),
            estate_users: HashSet::new(),
            estate_banned: HashSet::new(),
            estate_groups: HashSet::new(),
            terrain_textures: [Uuid::nil(); 4],
            elevation_bounds: [(10.0, 60.0), (10.0, 60.0), (10.0, 60.0), (10.0, 60.0)],
            water_height: 20.0,
        }
    }
}

// ---------------------------------------------------------------------------
// EstateSettingsUpdate — partial update struct
// ---------------------------------------------------------------------------

/// Partial update for [`EstateSettings`].  Only `Some` fields are applied.
#[derive(Debug, Default, Clone)]
pub struct EstateSettingsUpdate {
    pub estate_name: Option<String>,
    pub estate_owner: Option<Uuid>,
    pub abuse_email: Option<String>,
    pub deny_anonymous: Option<bool>,
    pub deny_identified: Option<bool>,
    pub allow_voice: Option<bool>,
    pub allow_direct_teleport: Option<bool>,
    pub deny_transacted: Option<bool>,
    pub deny_age_unverified: Option<bool>,
    pub deny_minors: Option<bool>,
    pub price_per_meter: Option<i32>,
    pub redirect_grid_x: Option<i32>,
    pub redirect_grid_y: Option<i32>,
    pub use_global_time: Option<bool>,
    pub fixed_sun: Option<bool>,
    pub sun_position: Option<f64>,
    pub covenant: Option<Uuid>,
    pub covenant_timestamp: Option<i64>,
    pub abuse_email_to_estate_owner: Option<bool>,
    pub water_height: Option<f64>,
}

impl EstateSettings {
    /// Apply a partial update, changing only fields that are `Some`.
    pub fn apply_update(&mut self, update: &EstateSettingsUpdate) {
        if let Some(ref v) = update.estate_name {
            self.estate_name = v.clone();
        }
        if let Some(v) = update.estate_owner {
            self.estate_owner = v;
        }
        if let Some(ref v) = update.abuse_email {
            self.abuse_email = v.clone();
        }
        if let Some(v) = update.deny_anonymous {
            self.deny_anonymous = v;
        }
        if let Some(v) = update.deny_identified {
            self.deny_identified = v;
        }
        if let Some(v) = update.allow_voice {
            self.allow_voice = v;
        }
        if let Some(v) = update.allow_direct_teleport {
            self.allow_direct_teleport = v;
        }
        if let Some(v) = update.deny_transacted {
            self.deny_transacted = v;
        }
        if let Some(v) = update.deny_age_unverified {
            self.deny_age_unverified = v;
        }
        if let Some(v) = update.deny_minors {
            self.deny_minors = v;
        }
        if let Some(v) = update.price_per_meter {
            self.price_per_meter = v;
        }
        if let Some(v) = update.redirect_grid_x {
            self.redirect_grid_x = v;
        }
        if let Some(v) = update.redirect_grid_y {
            self.redirect_grid_y = v;
        }
        if let Some(v) = update.use_global_time {
            self.use_global_time = v;
        }
        if let Some(v) = update.fixed_sun {
            self.fixed_sun = v;
        }
        if let Some(v) = update.sun_position {
            self.sun_position = v;
        }
        if let Some(v) = update.covenant {
            self.covenant = v;
        }
        if let Some(v) = update.covenant_timestamp {
            self.covenant_timestamp = v;
        }
        if let Some(v) = update.abuse_email_to_estate_owner {
            self.abuse_email_to_estate_owner = v;
        }
        if let Some(v) = update.water_height {
            self.water_height = v;
        }
    }
}

// ---------------------------------------------------------------------------
// TelehubManager
// ---------------------------------------------------------------------------

/// Manages the region telehub location and spawn points.
#[derive(Debug, Clone)]
pub struct TelehubManager {
    pub telehub_object_id: Uuid,
    pub spawn_points: Vec<[f32; 3]>,
}

impl Default for TelehubManager {
    fn default() -> Self {
        Self {
            telehub_object_id: Uuid::nil(),
            spawn_points: Vec::new(),
        }
    }
}

impl TelehubManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a spawn point to the telehub.
    pub fn add_spawn_point(&mut self, position: [f32; 3]) {
        self.spawn_points.push(position);
        tracing::debug!(
            position = ?position,
            count = self.spawn_points.len(),
            "added telehub spawn point"
        );
    }

    /// Remove a spawn point by index.  Returns `true` if the index was valid.
    pub fn remove_spawn_point(&mut self, index: usize) -> bool {
        if index < self.spawn_points.len() {
            self.spawn_points.remove(index);
            tracing::debug!(index, "removed telehub spawn point");
            true
        } else {
            false
        }
    }

    /// Returns the first spawn point, or the region centre default
    /// `(128, 128, 25)`.
    pub fn get_landing_point(&self) -> [f32; 3] {
        self.spawn_points
            .first()
            .copied()
            .unwrap_or([128.0, 128.0, 25.0])
    }
}

// ---------------------------------------------------------------------------
// EstateManagementModule
// ---------------------------------------------------------------------------

/// Estate-level management: access control, telehub, sun position, and
/// estate-wide administrative actions.
pub struct EstateManagementModule {
    pub settings: EstateSettings,
    pub telehub: TelehubManager,
    pub action_sender: Option<mpsc::Sender<EstateAction>>,
}

impl EstateManagementModule {
    pub fn new() -> Self {
        Self {
            settings: EstateSettings::default(),
            telehub: TelehubManager::new(),
            action_sender: None,
        }
    }

    // -- Access control ----------------------------------------------------

    /// Returns `true` if the agent is an estate manager (or the estate owner).
    pub fn is_estate_manager(&self, agent_id: Uuid) -> bool {
        agent_id == self.settings.estate_owner || self.settings.estate_managers.contains(&agent_id)
    }

    /// Returns `true` if the agent is on the estate ban list.
    pub fn is_banned(&self, agent_id: Uuid) -> bool {
        self.settings.estate_banned.contains(&agent_id)
    }

    /// Returns `true` if the agent has access to the estate.
    ///
    /// Access is granted when:
    /// - the agent is the estate owner or an estate manager, OR
    /// - the agent is in the users access list, OR
    /// - the agent belongs to a group in the groups access list
    ///   (group membership is not checked here; caller provides the
    ///    agent's group set via the groups list), OR
    /// - neither the users list nor the groups list are populated
    ///   (open estate).
    ///
    /// Access is always denied if the agent is banned.
    pub fn has_access(&self, agent_id: Uuid) -> bool {
        if self.is_banned(agent_id) {
            return false;
        }
        if self.is_estate_manager(agent_id) {
            return true;
        }
        if self.settings.estate_users.contains(&agent_id) {
            return true;
        }
        // If no explicit access lists are populated, the estate is open.
        if self.settings.estate_users.is_empty() && self.settings.estate_groups.is_empty() {
            return true;
        }
        false
    }

    /// Apply a partial settings update.
    pub fn update_settings(&mut self, update: EstateSettingsUpdate) {
        self.settings.apply_update(&update);
        tracing::debug!("estate settings updated");
    }

    // -- Estate actions -------------------------------------------------------

    fn send_action(&self, action: EstateAction) {
        if let Some(tx) = &self.action_sender {
            if let Err(e) = tx.send(action) {
                tracing::warn!("estate action channel closed: {e}");
            }
        } else {
            tracing::warn!("estate action_sender not wired — action dropped");
        }
    }

    /// Request a region restart after `delay_seconds`.
    pub fn restart_region(&self, delay_seconds: u32) {
        tracing::info!(delay_seconds, "restart_region requested");
        self.send_action(EstateAction::Restart { delay_seconds });
    }

    /// Kick a user from the region.
    pub fn kick_user(&self, agent_id: Uuid, message: &str) {
        tracing::info!(%agent_id, message, "kick_user requested");
        self.send_action(EstateAction::Kick {
            agent_id,
            message: message.to_string(),
        });
    }

    /// Teleport a user to their home region.
    pub fn teleport_home(&self, agent_id: Uuid) {
        tracing::info!(%agent_id, "teleport_home requested");
        self.send_action(EstateAction::TeleportHome { agent_id });
    }

    /// Broadcast a message to all agents in the region.
    pub fn send_region_message(&self, sender: &str, message: &str) {
        tracing::info!(sender, message, "send_region_message");
        self.send_action(EstateAction::RegionMessage {
            sender: sender.to_string(),
            message: message.to_string(),
        });
    }

    // -- Sun position ------------------------------------------------------

    /// Get the current estate sun position.
    pub fn get_sun_position(&self) -> f64 {
        self.settings.sun_position
    }

    /// Set the estate sun position.
    pub fn set_sun_position(&mut self, pos: f64) {
        self.settings.sun_position = pos;
        tracing::debug!(pos, "estate sun position set");
    }
}

impl Default for EstateManagementModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for EstateManagementModule {
    fn name(&self) -> &str {
        "EstateManagementModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::NonShared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        self.settings.allow_voice = config.get_bool("Estate", "AllowVoice", true);
        self.settings.allow_direct_teleport =
            config.get_bool("Estate", "AllowDirectTeleport", true);
        tracing::info!("EstateManagementModule initialised");
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "EstateManagementModule attached to region");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::info!(region = %scene.region_name, "EstateManagementModule detached from region");
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}
