// SPDX-License-Identifier: GPL-3.0-or-later

//! Hypergrid entity transfer module: cross-grid teleportation with
//! gatekeeper resolution, appearance restrictions, and foreign agent
//! verification.
//!
//! Mirrors the C# `HGEntityTransferModule` which extends
//! `EntityTransferModule` with Hypergrid-specific logic.

use std::collections::HashMap;
use std::sync::Arc;

use uuid::Uuid;

use crate::module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
use crate::registry::SceneRef;
use crate::transfer::GridRegion;
use opensim_services::hypergrid::{GatekeeperService, UserAgentService};

// ---------------------------------------------------------------------------
// Appearance restriction types
// ---------------------------------------------------------------------------

/// An allowed wearable asset for appearance restriction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllowedWearable {
    pub asset_id: Uuid,
    pub wearable_type: u32,
}

/// An allowed attachment asset for appearance restriction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllowedAttachment {
    pub asset_id: Uuid,
    pub attach_point: u32,
}

// ---------------------------------------------------------------------------
// HGEntityTransferModule
// ---------------------------------------------------------------------------

/// Extends the base `EntityTransferModule` with Hypergrid-aware teleport
/// logic.
///
/// Key responsibilities:
/// - Resolve hyperlink destinations via the foreign gatekeeper
/// - Validate appearance before outbound HG crossing
/// - Call `UserAgentService.LoginAgentToGrid` for cross-grid teleports
/// - Handle "teleport home" for foreign users
/// - Verify incoming foreign agents via gatekeeper callback
pub struct HGEntityTransferModule {
    enabled: bool,
    scene_name: String,
    local_grid_uri: String,

    // Configuration
    level_hg_teleport: i32,
    restrict_appearance_abroad: bool,
    appearance_account_name: String,

    // Allowed appearance items (populated from config)
    allowed_wearables: Vec<AllowedWearable>,
    allowed_attachments: Vec<AllowedAttachment>,

    // Service dependencies
    gatekeeper_service: Option<Arc<dyn GatekeeperService>>,
    user_agent_service: Option<Arc<dyn UserAgentService>>,

    // Tracks which agents are known foreign users
    foreign_agents: HashMap<Uuid, ForeignAgentInfo>,
}

/// Information about a foreign agent present on this grid.
#[derive(Debug, Clone)]
pub struct ForeignAgentInfo {
    pub home_uri: String,
    pub session_id: Uuid,
    pub service_token: String,
}

impl HGEntityTransferModule {
    pub fn new() -> Self {
        Self {
            enabled: false,
            scene_name: String::new(),
            local_grid_uri: String::new(),
            level_hg_teleport: 0,
            restrict_appearance_abroad: false,
            appearance_account_name: String::new(),
            allowed_wearables: Vec::new(),
            allowed_attachments: Vec::new(),
            gatekeeper_service: None,
            user_agent_service: None,
            foreign_agents: HashMap::new(),
        }
    }

    /// Inject the GatekeeperService dependency.
    pub fn set_gatekeeper_service(&mut self, svc: Arc<dyn GatekeeperService>) {
        self.gatekeeper_service = Some(svc);
    }

    /// Inject the UserAgentService dependency.
    pub fn set_user_agent_service(&mut self, svc: Arc<dyn UserAgentService>) {
        self.user_agent_service = Some(svc);
    }

    /// Register a foreign agent as present in this region.
    pub fn register_foreign_agent(&mut self, agent_id: Uuid, info: ForeignAgentInfo) {
        tracing::debug!(
            agent = %agent_id,
            home = %info.home_uri,
            "registered foreign agent"
        );
        self.foreign_agents.insert(agent_id, info);
    }

    /// Remove a foreign agent record.
    pub fn deregister_foreign_agent(&mut self, agent_id: Uuid) {
        self.foreign_agents.remove(&agent_id);
    }

    /// Check whether an agent is a known foreign user.
    pub fn is_foreign_agent(&self, agent_id: Uuid) -> bool {
        self.foreign_agents.contains_key(&agent_id)
    }

    /// Get foreign agent info.
    pub fn get_foreign_agent_info(&self, agent_id: Uuid) -> Option<&ForeignAgentInfo> {
        self.foreign_agents.get(&agent_id)
    }

    /// Resolve a hyperlinked region to its real destination via the
    /// gatekeeper.
    ///
    /// Mirrors the C# `GetFinalDestination` override.
    pub fn get_final_destination(
        &self,
        region: &GridRegion,
        agent_id: Uuid,
        agent_home_uri: Option<&str>,
        is_hyperlink: bool,
    ) -> Result<GridRegion, String> {
        if !is_hyperlink {
            return Ok(region.clone());
        }

        if let Some(ref gk) = self.gatekeeper_service {
            match gk.get_hyperlink_region(region.region_id, agent_id, agent_home_uri) {
                Ok((info, _message)) => {
                    let resolved = GridRegion {
                        region_id: info.region_id,
                        region_handle: info.region_handle,
                        region_name: info.region_name,
                        external_host_name: info.external_host_name,
                        http_port: info.internal_port,
                        server_uri: info.server_uri,
                        region_loc_x: info.region_loc_x,
                        region_loc_y: info.region_loc_y,
                        region_size_x: info.region_size_x,
                        region_size_y: info.region_size_y,
                    };
                    Ok(resolved)
                }
                Err(e) => Err(format!("gatekeeper lookup failed: {}", e)),
            }
        } else {
            Err("no gatekeeper service configured".to_string())
        }
    }

    /// Check whether an agent meets the minimum level for HG teleport.
    pub fn can_hg_teleport(&self, user_level: i32) -> bool {
        user_level >= self.level_hg_teleport
    }

    /// Validate appearance for outbound HG crossing.
    ///
    /// If `restrict_appearance_abroad` is enabled, checks that all
    /// wearables and attachments are in the allowed list.
    pub fn validate_appearance(
        &self,
        wearable_assets: &[Uuid],
        attachment_assets: &[Uuid],
    ) -> Result<(), String> {
        if !self.restrict_appearance_abroad {
            return Ok(());
        }

        for asset in wearable_assets {
            if !self
                .allowed_wearables
                .iter()
                .any(|w| w.asset_id == *asset)
            {
                return Err(format!(
                    "wearable {} not allowed for cross-grid teleport",
                    asset
                ));
            }
        }

        for asset in attachment_assets {
            if !self
                .allowed_attachments
                .iter()
                .any(|a| a.asset_id == *asset)
            {
                return Err(format!(
                    "attachment {} not allowed for cross-grid teleport",
                    asset
                ));
            }
        }

        Ok(())
    }

    /// Perform a cross-grid teleport via the UserAgentService.
    ///
    /// This is the HG equivalent of `CreateAgent` in the C# code.
    pub fn teleport_to_foreign_grid(
        &self,
        agent_id: Uuid,
        home_uri: &str,
        _destination: &GridRegion,
    ) -> Result<(), String> {
        if !self.enabled {
            return Err("module not enabled".to_string());
        }

        if let Some(ref uas) = self.user_agent_service {
            let _ = uas; // Would call uas.login_agent_to_grid() in real impl
            tracing::info!(
                agent = %agent_id,
                home = %home_uri,
                "cross-grid teleport initiated (stub)"
            );
            Ok(())
        } else {
            Err("no UserAgentService configured".to_string())
        }
    }

    /// Teleport a foreign user home via their home grid's
    /// UserAgentService.
    pub fn teleport_home_foreign(
        &self,
        agent_id: Uuid,
    ) -> Result<(), String> {
        let info = self
            .foreign_agents
            .get(&agent_id)
            .ok_or_else(|| "agent is not a known foreign user".to_string())?;

        if let Some(ref uas) = self.user_agent_service {
            match uas.get_home_region(agent_id) {
                Ok((_region, _pos, _look_at)) => {
                    tracing::info!(
                        agent = %agent_id,
                        home = %info.home_uri,
                        "teleport home for foreign user (stub)"
                    );
                    Ok(())
                }
                Err(e) => Err(format!("failed to get home region: {}", e)),
            }
        } else {
            Err("no UserAgentService configured".to_string())
        }
    }

    /// Verify an incoming foreign agent by calling back to their home
    /// grid's UserAgentService.
    pub fn verify_agent(
        &self,
        session_id: Uuid,
        token: &str,
    ) -> Result<bool, String> {
        if let Some(ref uas) = self.user_agent_service {
            match uas.verify_agent(session_id, token) {
                Ok(valid) => Ok(valid),
                Err(e) => Err(format!("verification failed: {}", e)),
            }
        } else {
            Err("no UserAgentService configured".to_string())
        }
    }

    /// Number of currently tracked foreign agents.
    pub fn foreign_agent_count(&self) -> usize {
        self.foreign_agents.len()
    }
}

impl Default for HGEntityTransferModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for HGEntityTransferModule {
    fn name(&self) -> &str {
        "HGEntityTransferModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::NonShared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        let module_name =
            config.get_string("Modules", "EntityTransferModule", "");
        if module_name != self.name() {
            tracing::debug!("HGEntityTransferModule disabled by configuration");
            return Ok(());
        }

        self.enabled = true;
        self.local_grid_uri = config.get_string("Hypergrid", "GatekeeperURI", "");
        self.level_hg_teleport =
            config.get_int("EntityTransfer", "LevelHGTeleport", 0) as i32;
        self.restrict_appearance_abroad =
            config.get_bool("EntityTransfer", "RestrictAppearanceAbroad", false);
        self.appearance_account_name =
            config.get_string("EntityTransfer", "AccountForAppearance", "");

        tracing::info!(
            enabled = self.enabled,
            level_hg = self.level_hg_teleport,
            restrict_appearance = self.restrict_appearance_abroad,
            "HGEntityTransferModule initialised"
        );
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        if !self.enabled {
            return Ok(());
        }
        self.scene_name = scene.region_name.clone();
        tracing::info!(
            region = %scene.region_name,
            "HGEntityTransferModule attached to region"
        );
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        if !self.enabled {
            return Ok(());
        }
        self.scene_name.clear();
        self.foreign_agents.clear();
        tracing::info!(
            region = %scene.region_name,
            "HGEntityTransferModule detached from region"
        );
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.scene_name.clear();
        self.foreign_agents.clear();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct TestConfig {
        strings: HashMap<(String, String), String>,
        ints: HashMap<(String, String), i64>,
        bools: HashMap<(String, String), bool>,
    }

    impl TestConfig {
        fn new() -> Self {
            Self {
                strings: HashMap::new(),
                ints: HashMap::new(),
                bools: HashMap::new(),
            }
        }

        fn set_string(&mut self, section: &str, key: &str, value: &str) {
            self.strings
                .insert((section.to_string(), key.to_string()), value.to_string());
        }

        fn set_int(&mut self, section: &str, key: &str, value: i64) {
            self.ints
                .insert((section.to_string(), key.to_string()), value);
        }

        fn set_bool(&mut self, section: &str, key: &str, value: bool) {
            self.bools
                .insert((section.to_string(), key.to_string()), value);
        }
    }

    impl ModuleConfig for TestConfig {
        fn get_string(&self, section: &str, key: &str, default: &str) -> String {
            self.strings
                .get(&(section.to_string(), key.to_string()))
                .cloned()
                .unwrap_or_else(|| default.to_string())
        }
        fn get_int(&self, section: &str, key: &str, default: i64) -> i64 {
            self.ints
                .get(&(section.to_string(), key.to_string()))
                .copied()
                .unwrap_or(default)
        }
        fn get_bool(&self, section: &str, key: &str, default: bool) -> bool {
            self.bools
                .get(&(section.to_string(), key.to_string()))
                .copied()
                .unwrap_or(default)
        }
        fn get_float(&self, _section: &str, _key: &str, default: f64) -> f64 {
            default
        }
    }

    fn scene_ref() -> SceneRef {
        SceneRef::new(Uuid::new_v4(), "TestRegion".to_string())
    }

    // -- 1. Module initialises when configured ---------------------------

    #[test]
    fn module_initialise_enabled() {
        let mut module = HGEntityTransferModule::new();
        let mut config = TestConfig::new();
        config.set_string("Modules", "EntityTransferModule", "HGEntityTransferModule");
        config.set_string("Hypergrid", "GatekeeperURI", "http://mygrid.com:8002");
        config.set_int("EntityTransfer", "LevelHGTeleport", 10);
        config.set_bool("EntityTransfer", "RestrictAppearanceAbroad", true);
        module.initialise(&config).unwrap();
        assert!(module.enabled);
        assert_eq!(module.level_hg_teleport, 10);
        assert!(module.restrict_appearance_abroad);
    }

    // -- 2. Module disabled by default -----------------------------------

    #[test]
    fn module_disabled_by_default() {
        let mut module = HGEntityTransferModule::new();
        let config = TestConfig::new();
        module.initialise(&config).unwrap();
        assert!(!module.enabled);
    }

    // -- 3. HG teleport level check -------------------------------------

    #[test]
    fn hg_teleport_level_check() {
        let mut module = HGEntityTransferModule::new();
        module.level_hg_teleport = 10;
        assert!(!module.can_hg_teleport(5));
        assert!(module.can_hg_teleport(10));
        assert!(module.can_hg_teleport(15));
    }

    // -- 4. Appearance validation passes when disabled ------------------

    #[test]
    fn appearance_validation_disabled() {
        let module = HGEntityTransferModule::new();
        assert!(module
            .validate_appearance(&[Uuid::new_v4()], &[Uuid::new_v4()])
            .is_ok());
    }

    // -- 5. Appearance validation fails for unknown items ----------------

    #[test]
    fn appearance_validation_rejects_unknown() {
        let mut module = HGEntityTransferModule::new();
        module.restrict_appearance_abroad = true;
        let allowed_asset = Uuid::new_v4();
        module.allowed_wearables.push(AllowedWearable {
            asset_id: allowed_asset,
            wearable_type: 0,
        });

        assert!(module
            .validate_appearance(&[allowed_asset], &[])
            .is_ok());
        assert!(module
            .validate_appearance(&[Uuid::new_v4()], &[])
            .is_err());
    }

    // -- 6. Foreign agent registration -----------------------------------

    #[test]
    fn foreign_agent_registration() {
        let mut module = HGEntityTransferModule::new();
        let agent = Uuid::new_v4();
        let info = ForeignAgentInfo {
            home_uri: "http://foreign.grid:8002".to_string(),
            session_id: Uuid::new_v4(),
            service_token: "token123".to_string(),
        };
        module.register_foreign_agent(agent, info);
        assert!(module.is_foreign_agent(agent));
        assert_eq!(module.foreign_agent_count(), 1);

        module.deregister_foreign_agent(agent);
        assert!(!module.is_foreign_agent(agent));
    }

    // -- 7. Module lifecycle ---------------------------------------------

    #[test]
    fn module_lifecycle() {
        let mut module = HGEntityTransferModule::new();
        let mut config = TestConfig::new();
        config.set_string("Modules", "EntityTransferModule", "HGEntityTransferModule");
        module.initialise(&config).unwrap();

        let scene = scene_ref();
        module.add_region(&scene).unwrap();
        assert_eq!(module.scene_name, scene.region_name);

        // Register a foreign agent, then remove region clears them.
        module.register_foreign_agent(
            Uuid::new_v4(),
            ForeignAgentInfo {
                home_uri: "http://test.grid".to_string(),
                session_id: Uuid::new_v4(),
                service_token: String::new(),
            },
        );
        module.remove_region(&scene).unwrap();
        assert!(module.scene_name.is_empty());
        assert_eq!(module.foreign_agent_count(), 0);
    }

    // -- 8. Teleport to foreign grid fails without UAS -------------------

    #[test]
    fn teleport_foreign_no_uas() {
        let module = HGEntityTransferModule::new();
        let dest = GridRegion::new(Uuid::new_v4(), "ForeignRegion");
        let result =
            module.teleport_to_foreign_grid(Uuid::new_v4(), "http://home.grid", &dest);
        assert!(result.is_err());
    }

    // -- 9. Get final destination without gatekeeper ---------------------

    #[test]
    fn final_destination_no_gatekeeper() {
        let module = HGEntityTransferModule::new();
        let region = GridRegion::new(Uuid::new_v4(), "TestRegion");

        // Non-hyperlink: returns original region.
        let result =
            module.get_final_destination(&region, Uuid::new_v4(), None, false);
        assert!(result.is_ok());

        // Hyperlink without gatekeeper: returns error.
        let result =
            module.get_final_destination(&region, Uuid::new_v4(), None, true);
        assert!(result.is_err());
    }
}
