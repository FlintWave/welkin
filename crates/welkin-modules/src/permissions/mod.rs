// SPDX-License-Identifier: LGPL-3.0-or-later

//! Default permissions module — Rust port of the C# `DefaultPermissionsModule`.
//!
//! Implements the region permission model: bypass flags, god-mode detection,
//! administrator detection, and all per-action permission checks (objects,
//! scripts, terrain, parcels, estate).

pub mod checks;

use std::collections::HashSet;

use uuid::Uuid;

use crate::module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
use crate::registry::SceneRef;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// All boolean config knobs read from `[Startup]` / `[Permissions]` sections.
///
/// Mirrors the C# fields `m_bypassPermissions`, `m_propagatePermissions`, etc.
#[derive(Debug, Clone)]
pub struct PermissionsConfig {
    /// When true every permission check returns `bypass_permissions_value`.
    /// Corresponds to `serverside_object_permissions = false` in the INI.
    pub bypass_permissions: bool,

    /// The value returned by all checks when `bypass_permissions` is true.
    pub bypass_permissions_value: bool,

    /// Whether to propagate permissions on rez / derez.
    pub propagate_permissions: bool,

    /// Allow grid-level gods (UserLevel >= 200) to act as admins.
    pub allow_grid_gods: bool,

    /// The estate owner is treated as an administrator.
    pub region_owner_is_god: bool,

    /// Estate managers are treated as administrators.
    pub region_manager_is_god: bool,

    /// Only grid gods may be admins (overrides estate-owner / manager flags).
    pub force_grid_gods_only: bool,

    /// God-mode is automatically enabled for admins (no need to request it).
    pub automatic_gods: bool,

    /// Admins can perform admin actions without explicitly entering god-mode.
    pub implicit_gods: bool,

    /// Who is allowed to create scripts — `All` or `Administrators`.
    pub allowed_script_creators: UserSet,

    /// Who is allowed to edit scripts — `All` or `Administrators`.
    pub allowed_script_editors: UserSet,
}

impl Default for PermissionsConfig {
    fn default() -> Self {
        Self {
            bypass_permissions: false,
            bypass_permissions_value: true,
            propagate_permissions: true,
            allow_grid_gods: false,
            region_owner_is_god: true,
            region_manager_is_god: false,
            force_grid_gods_only: false,
            automatic_gods: false,
            implicit_gods: false,
            allowed_script_creators: UserSet::All,
            allowed_script_editors: UserSet::All,
        }
    }
}

/// Who may create / edit scripts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserSet {
    All,
    Administrators,
}

// ---------------------------------------------------------------------------
// DefaultPermissionsModule
// ---------------------------------------------------------------------------

/// The primary permissions module for a region.
///
/// Each region has its own instance (non-shared).  During `initialise()` the
/// module reads configuration; during `add_region()` it picks up estate
/// ownership data from the [`SceneRef`].
pub struct DefaultPermissionsModule {
    /// Runtime configuration knobs.
    pub config: PermissionsConfig,

    /// The estate owner UUID.
    pub estate_owner: Uuid,

    /// Estate managers.
    pub estate_managers: HashSet<Uuid>,

    /// Explicitly configured god / admin UUIDs (grid gods).
    pub god_users: HashSet<Uuid>,

    /// Region ID (populated on `add_region`).
    region_id: Uuid,
}

impl DefaultPermissionsModule {
    pub fn new() -> Self {
        Self {
            config: PermissionsConfig::default(),
            estate_owner: Uuid::nil(),
            estate_managers: HashSet::new(),
            god_users: HashSet::new(),
            region_id: Uuid::nil(),
        }
    }

    // -- Admin / god detection ------------------------------------------------

    /// Is the user a grid-level administrator?
    ///
    /// Checks the explicit `god_users` set.  In the full implementation this
    /// also checks `UserAccount.UserLevel >= 200` when `allow_grid_gods` is
    /// enabled; here we use the `god_users` set as the equivalent.
    pub fn is_grid_administrator(&self, agent_id: Uuid) -> bool {
        if agent_id.is_nil() {
            return false;
        }
        if self.config.allow_grid_gods && self.god_users.contains(&agent_id) {
            return true;
        }
        false
    }

    /// Is the user an estate manager (or estate owner)?
    pub fn is_estate_manager(&self, agent_id: Uuid) -> bool {
        if agent_id.is_nil() {
            return false;
        }
        if agent_id == self.estate_owner {
            return true;
        }
        self.estate_managers.contains(&agent_id)
    }

    /// Is the user regarded as an administrator?
    ///
    /// Checks (in order): estate owner (if `region_owner_is_god`), estate
    /// manager (if `region_manager_is_god`), and grid administrator.
    pub fn is_administrator(&self, agent_id: Uuid) -> bool {
        if agent_id.is_nil() {
            return false;
        }

        if self.config.region_owner_is_god && agent_id == self.estate_owner {
            return true;
        }

        if self.config.region_manager_is_god && self.is_estate_manager(agent_id) {
            return true;
        }

        if self.is_grid_administrator(agent_id) {
            return true;
        }

        false
    }

    /// Is the user in god-mode?
    ///
    /// In the full C# implementation this checks the presence's `IsGod` flag
    /// which is set when a user with UserLevel >= 200 requests god mode.
    /// Here we simplify: a user is "god" if they are an administrator and
    /// either `automatic_gods` or `implicit_gods` is set, or if they are in
    /// the explicit `god_users` set.
    pub fn is_god(&self, agent_id: Uuid) -> bool {
        if agent_id.is_nil() {
            return false;
        }

        // Explicit grid gods are always considered to be in god-mode.
        if self.god_users.contains(&agent_id) {
            return true;
        }

        // With automatic or implicit gods, any admin is treated as god.
        if (self.config.automatic_gods || self.config.implicit_gods)
            && self.is_administrator(agent_id)
        {
            return true;
        }

        false
    }

    /// Generic estate permission check: estate manager or administrator.
    pub fn generic_estate_permission(&self, agent_id: Uuid) -> bool {
        if self.is_estate_manager(agent_id) {
            return true;
        }
        if self.is_administrator(agent_id) {
            return true;
        }
        false
    }
}

impl Default for DefaultPermissionsModule {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// RegionModule implementation
// ---------------------------------------------------------------------------

impl RegionModule for DefaultPermissionsModule {
    fn name(&self) -> &str {
        "DefaultPermissionsModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::NonShared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        // Read from both [Startup] and [Permissions] sections, preferring
        // [Permissions] (C# checks both with `GetConfigVarFromSections`).
        // Our ModuleConfig always takes a single section, so we read
        // [Permissions] first; the caller should have merged both.

        let section = "Permissions";

        // bypass_permissions is the inverse of serverside_object_permissions.
        let serverside = config.get_bool(section, "serverside_object_permissions", true);
        self.config.bypass_permissions = !serverside;

        self.config.propagate_permissions = config.get_bool(section, "propagate_permissions", true);
        self.config.allow_grid_gods = config.get_bool(section, "allow_grid_gods", false);

        self.config.force_grid_gods_only = config.get_bool(section, "force_grid_gods_only", false);

        if self.config.force_grid_gods_only {
            // When forcing grid gods only, estate owner/manager are NOT admins
            // and allow_grid_gods is forced on.
            self.config.allow_grid_gods = true;
            self.config.region_owner_is_god = false;
            self.config.region_manager_is_god = false;
        } else {
            self.config.region_owner_is_god = config.get_bool(section, "region_owner_is_god", true);
            self.config.region_manager_is_god =
                config.get_bool(section, "region_manager_is_god", false);
        }

        self.config.automatic_gods = config.get_bool(section, "automatic_gods", false);
        self.config.implicit_gods = config.get_bool(section, "implicit_gods", false);
        if self.config.implicit_gods {
            self.config.automatic_gods = false;
        }

        // Script creator / editor sets.
        let creators = config.get_string(section, "allowed_script_creators", "all");
        self.config.allowed_script_creators = match creators.to_lowercase().as_str() {
            "administrators" | "gods" => UserSet::Administrators,
            _ => UserSet::All,
        };

        let editors = config.get_string(section, "allowed_script_editors", "all");
        self.config.allowed_script_editors = match editors.to_lowercase().as_str() {
            "administrators" | "gods" => UserSet::Administrators,
            _ => UserSet::All,
        };

        tracing::info!(
            bypass = self.config.bypass_permissions,
            propagate = self.config.propagate_permissions,
            "DefaultPermissionsModule initialised"
        );

        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.region_id = scene.region_id;
        tracing::debug!(
            region = %scene.region_name,
            "DefaultPermissionsModule attached to region"
        );
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        tracing::debug!(
            region = %scene.region_name,
            "DefaultPermissionsModule detached from region"
        );
        self.region_id = Uuid::nil();
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}
