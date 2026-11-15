// SPDX-License-Identifier: GPL-3.0-or-later

//! Concierge module: lobby/welcome announcements and HTTP broker
//! notifications for avatar enter/leave events.
//!
//! Mirrors the C# `ConciergeModule` from
//! `OpenSim/Region/OptionalModules/Avatar/Concierge/ConciergeModule.cs`.

use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use crate::events::ChatMessageEvent;
use crate::module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
use crate::registry::SceneRef;

// ---------------------------------------------------------------------------
// ConciergeArea — tracks attendance for a parcel or region
// ---------------------------------------------------------------------------

/// Identifies a concierge attendance area (a region or a parcel within one).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ConciergeArea {
    /// Whole region.
    Region(Uuid),
    /// A specific parcel within a region.
    Parcel { region_id: Uuid, parcel_id: Uuid },
}

// ---------------------------------------------------------------------------
// ConciergeModule
// ---------------------------------------------------------------------------

/// Region module that sends welcome/goodbye announcements and broker
/// notifications when avatars enter or leave concierge areas.
///
/// Configurable per-region and per-parcel welcome messages support the
/// following substitution variables:
/// - `{0}` — avatar display name
/// - `{1}` — region name
/// - `{2}` — current attendance count in the area
pub struct ConciergeModule {
    /// Whether the module is enabled.
    enabled: bool,
    /// Default welcome message template.
    welcome_message: String,
    /// Default goodbye message template.
    goodbye_message: String,
    /// Per-area custom welcome messages.
    area_messages: HashMap<ConciergeArea, String>,
    /// Attendance tracking: area -> set of agent UUIDs.
    attendance: HashMap<ConciergeArea, HashSet<Uuid>>,
    /// Optional HTTP broker URL to POST enter/leave notifications to.
    broker_url: String,
    /// The scene this module is attached to.
    scene: Option<SceneRef>,
}

impl ConciergeModule {
    pub fn new() -> Self {
        Self {
            enabled: false,
            welcome_message: "Welcome to {1}, {0}!".to_string(),
            goodbye_message: "{0} has left {1}.".to_string(),
            area_messages: HashMap::new(),
            attendance: HashMap::new(),
            broker_url: String::new(),
            scene: None,
        }
    }

    /// Set a custom welcome message for a specific area.
    pub fn set_area_message(&mut self, area: ConciergeArea, message: String) {
        self.area_messages.insert(area, message);
    }

    /// Get the welcome message for an area, falling back to the default.
    pub fn get_welcome_message(&self, area: &ConciergeArea) -> &str {
        self.area_messages
            .get(area)
            .map(|s| s.as_str())
            .unwrap_or(&self.welcome_message)
    }

    /// Substitute variables in a message template.
    ///
    /// - `{0}` = avatar_name
    /// - `{1}` = region_name
    /// - `{2}` = count (attendance in the area)
    pub fn substitute_message(
        template: &str,
        avatar_name: &str,
        region_name: &str,
        count: usize,
    ) -> String {
        template
            .replace("{0}", avatar_name)
            .replace("{1}", region_name)
            .replace("{2}", &count.to_string())
    }

    /// Record an avatar entering a concierge area.
    ///
    /// Returns the formatted welcome message if the module is enabled and the
    /// avatar was not already tracked in the area.
    pub fn on_avatar_enter(
        &mut self,
        area: ConciergeArea,
        agent_id: Uuid,
        avatar_name: &str,
        region_name: &str,
    ) -> Option<String> {
        if !self.enabled {
            return None;
        }

        let agents = self.attendance.entry(area.clone()).or_default();
        if !agents.insert(agent_id) {
            // Already present — no welcome.
            return None;
        }

        let count = agents.len();
        let template = self.get_welcome_message(&area);
        let msg = Self::substitute_message(template, avatar_name, region_name, count);

        // Broadcast the welcome via the scene event bus.
        if let Some(ref scene) = self.scene {
            let evt = ChatMessageEvent {
                region_id: scene.region_id,
                sender_id: Uuid::nil(),
                message: msg.clone(),
                channel: 0,
            };
            let _ = scene.events.on_chat_message.send(evt);
        }

        // Fire HTTP broker notification (stub).
        if !self.broker_url.is_empty() {
            self.notify_broker("enter", agent_id, avatar_name, region_name, count);
        }

        Some(msg)
    }

    /// Record an avatar leaving a concierge area.
    ///
    /// Returns the formatted goodbye message if the module is enabled and the
    /// avatar was tracked in the area.
    pub fn on_avatar_leave(
        &mut self,
        area: ConciergeArea,
        agent_id: Uuid,
        avatar_name: &str,
        region_name: &str,
    ) -> Option<String> {
        if !self.enabled {
            return None;
        }

        let agents = self.attendance.get_mut(&area)?;
        if !agents.remove(&agent_id) {
            return None;
        }

        let count = agents.len();
        let msg = Self::substitute_message(&self.goodbye_message, avatar_name, region_name, count);

        // Broadcast the goodbye via the scene event bus.
        if let Some(ref scene) = self.scene {
            let evt = ChatMessageEvent {
                region_id: scene.region_id,
                sender_id: Uuid::nil(),
                message: msg.clone(),
                channel: 0,
            };
            let _ = scene.events.on_chat_message.send(evt);
        }

        if !self.broker_url.is_empty() {
            self.notify_broker("leave", agent_id, avatar_name, region_name, count);
        }

        Some(msg)
    }

    /// Get the attendance count for an area.
    pub fn attendance_count(&self, area: &ConciergeArea) -> usize {
        self.attendance.get(area).map(|s| s.len()).unwrap_or(0)
    }

    /// Get the set of agents in an area.
    pub fn agents_in_area(&self, area: &ConciergeArea) -> Vec<Uuid> {
        self.attendance
            .get(area)
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default()
    }

    /// HTTP broker notification stub.
    ///
    /// In a full implementation this would POST a JSON payload to
    /// `self.broker_url`.
    fn notify_broker(
        &self,
        action: &str,
        agent_id: Uuid,
        avatar_name: &str,
        region_name: &str,
        count: usize,
    ) {
        tracing::debug!(
            url = %self.broker_url,
            action,
            agent = %agent_id,
            name = avatar_name,
            region = region_name,
            count,
            "ConciergeModule: broker notification (stub)"
        );
    }
}

impl Default for ConciergeModule {
    fn default() -> Self {
        Self::new()
    }
}

impl RegionModule for ConciergeModule {
    fn name(&self) -> &str {
        "ConciergeModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::NonShared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        self.enabled = config.get_bool("Concierge", "enabled", false);
        self.welcome_message =
            config.get_string("Concierge", "welcome_message", "Welcome to {1}, {0}!");
        self.goodbye_message =
            config.get_string("Concierge", "goodbye_message", "{0} has left {1}.");
        self.broker_url = config.get_string("Concierge", "broker", "");
        tracing::info!(
            enabled = self.enabled,
            broker = %self.broker_url,
            "ConciergeModule initialised"
        );
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.scene = Some(scene.clone());
        tracing::info!(region = %scene.region_name, "ConciergeModule attached to region");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.attendance.clear();
        self.scene = None;
        tracing::info!(region = %scene.region_name, "ConciergeModule detached from region");
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        self.attendance.clear();
        self.scene = None;
        Ok(())
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::ModuleConfig;
    use std::collections::HashMap as StdHashMap;

    struct TestConfig {
        strings: StdHashMap<(String, String), String>,
        bools: StdHashMap<(String, String), bool>,
    }

    impl TestConfig {
        fn new() -> Self {
            Self {
                strings: StdHashMap::new(),
                bools: StdHashMap::new(),
            }
        }

        fn set_bool(&mut self, section: &str, key: &str, value: bool) {
            self.bools
                .insert((section.to_string(), key.to_string()), value);
        }

        fn set_string(&mut self, section: &str, key: &str, value: &str) {
            self.strings
                .insert((section.to_string(), key.to_string()), value.to_string());
        }
    }

    impl ModuleConfig for TestConfig {
        fn get_string(&self, section: &str, key: &str, default: &str) -> String {
            self.strings
                .get(&(section.to_string(), key.to_string()))
                .cloned()
                .unwrap_or_else(|| default.to_string())
        }
        fn get_int(&self, _section: &str, _key: &str, default: i64) -> i64 {
            default
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

    fn test_scene() -> SceneRef {
        SceneRef::new(Uuid::new_v4(), "TestRegion".to_string())
    }

    fn setup_module() -> ConciergeModule {
        let mut module = ConciergeModule::new();
        let mut config = TestConfig::new();
        config.set_bool("Concierge", "enabled", true);
        module.initialise(&config).unwrap();
        let scene = test_scene();
        module.add_region(&scene).unwrap();
        module
    }

    // -- Test 1: welcome message on enter ------------------------------------

    #[test]
    fn welcome_message_on_enter() {
        let mut module = setup_module();
        let region_id = module.scene.as_ref().unwrap().region_id;
        let area = ConciergeArea::Region(region_id);
        let agent = Uuid::new_v4();

        let msg = module
            .on_avatar_enter(area.clone(), agent, "Alice", "TestRegion")
            .unwrap();
        assert_eq!(msg, "Welcome to TestRegion, Alice!");
        assert_eq!(module.attendance_count(&area), 1);
    }

    // -- Test 2: goodbye message on leave ------------------------------------

    #[test]
    fn goodbye_message_on_leave() {
        let mut module = setup_module();
        let region_id = module.scene.as_ref().unwrap().region_id;
        let area = ConciergeArea::Region(region_id);
        let agent = Uuid::new_v4();

        module.on_avatar_enter(area.clone(), agent, "Bob", "TestRegion");
        let msg = module
            .on_avatar_leave(area.clone(), agent, "Bob", "TestRegion")
            .unwrap();
        assert_eq!(msg, "Bob has left TestRegion.");
        assert_eq!(module.attendance_count(&area), 0);
    }

    // -- Test 3: variable substitution ---------------------------------------

    #[test]
    fn variable_substitution() {
        let msg = ConciergeModule::substitute_message(
            "Hello {0}, welcome to {1}! There are {2} people here.",
            "Charlie",
            "SandboxRegion",
            5,
        );
        assert_eq!(
            msg,
            "Hello Charlie, welcome to SandboxRegion! There are 5 people here."
        );
    }

    // -- Test 4: custom area messages ----------------------------------------

    #[test]
    fn custom_area_messages() {
        let mut module = setup_module();
        let region_id = module.scene.as_ref().unwrap().region_id;
        let area = ConciergeArea::Region(region_id);

        module.set_area_message(
            area.clone(),
            "Greetings {0}, you've entered {1}!".to_string(),
        );

        let agent = Uuid::new_v4();
        let msg = module
            .on_avatar_enter(area, agent, "Dave", "TestRegion")
            .unwrap();
        assert_eq!(msg, "Greetings Dave, you've entered TestRegion!");
    }

    // -- Test 5: duplicate enter is ignored -----------------------------------

    #[test]
    fn duplicate_enter_ignored() {
        let mut module = setup_module();
        let region_id = module.scene.as_ref().unwrap().region_id;
        let area = ConciergeArea::Region(region_id);
        let agent = Uuid::new_v4();

        let first = module.on_avatar_enter(area.clone(), agent, "Eve", "TestRegion");
        assert!(first.is_some());

        let second = module.on_avatar_enter(area.clone(), agent, "Eve", "TestRegion");
        assert!(second.is_none());
        assert_eq!(module.attendance_count(&area), 1);
    }

    // -- Test 6: leave without enter is ignored -------------------------------

    #[test]
    fn leave_without_enter_ignored() {
        let mut module = setup_module();
        let region_id = module.scene.as_ref().unwrap().region_id;
        let area = ConciergeArea::Region(region_id);
        let agent = Uuid::new_v4();

        let msg = module.on_avatar_leave(area, agent, "Nobody", "TestRegion");
        assert!(msg.is_none());
    }

    // -- Test 7: disabled module returns None ---------------------------------

    #[test]
    fn disabled_module_returns_none() {
        let mut module = ConciergeModule::new();
        // Don't enable it.
        let config = TestConfig::new();
        module.initialise(&config).unwrap();
        let scene = test_scene();
        module.add_region(&scene).unwrap();

        let area = ConciergeArea::Region(scene.region_id);
        let agent = Uuid::new_v4();
        assert!(
            module
                .on_avatar_enter(area, agent, "Ghost", "TestRegion")
                .is_none()
        );
    }

    // -- Test 8: parcel-level tracking ----------------------------------------

    #[test]
    fn parcel_level_tracking() {
        let mut module = setup_module();
        let region_id = module.scene.as_ref().unwrap().region_id;
        let parcel_a = ConciergeArea::Parcel {
            region_id,
            parcel_id: Uuid::new_v4(),
        };
        let parcel_b = ConciergeArea::Parcel {
            region_id,
            parcel_id: Uuid::new_v4(),
        };

        let agent1 = Uuid::new_v4();
        let agent2 = Uuid::new_v4();

        module.on_avatar_enter(parcel_a.clone(), agent1, "P1", "TestRegion");
        module.on_avatar_enter(parcel_b.clone(), agent2, "P2", "TestRegion");

        assert_eq!(module.attendance_count(&parcel_a), 1);
        assert_eq!(module.attendance_count(&parcel_b), 1);

        let agents = module.agents_in_area(&parcel_a);
        assert_eq!(agents.len(), 1);
        assert!(agents.contains(&agent1));
    }

    // -- Test 9: broker URL configuration -------------------------------------

    #[test]
    fn broker_url_configuration() {
        let mut module = ConciergeModule::new();
        let mut config = TestConfig::new();
        config.set_bool("Concierge", "enabled", true);
        config.set_string("Concierge", "broker", "http://example.com/concierge");
        module.initialise(&config).unwrap();
        assert_eq!(module.broker_url, "http://example.com/concierge");
    }

    // -- Test 10: RegionModule lifecycle --------------------------------------

    #[test]
    fn concierge_module_lifecycle() {
        let mut module = ConciergeModule::new();
        let mut config = TestConfig::new();
        config.set_bool("Concierge", "enabled", true);
        let scene = test_scene();

        module.initialise(&config).unwrap();
        module.add_region(&scene).unwrap();
        assert_eq!(module.name(), "ConciergeModule");
        assert_eq!(module.module_type(), ModuleType::NonShared);

        // Add attendance, then remove_region clears it.
        let area = ConciergeArea::Region(scene.region_id);
        module.on_avatar_enter(area.clone(), Uuid::new_v4(), "X", "TestRegion");
        assert_eq!(module.attendance_count(&area), 1);

        module.remove_region(&scene).unwrap();
        assert_eq!(module.attendance_count(&area), 0);

        module.close().unwrap();
    }
}
