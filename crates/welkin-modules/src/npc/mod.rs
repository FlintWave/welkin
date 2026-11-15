// SPDX-License-Identifier: GPL-3.0-or-later

//! NPC (Non-Player Character) module: creates, manages, and removes scripted
//! avatar presences in a region.
//!
//! Mirrors the C# `NPCModule` from `OpenSim/Region/OptionalModules/World/NPC/`
//! and the `INPCModule` interface from `OpenSim/Region/Framework/Interfaces/`.

pub mod npc_avatar;

use std::collections::HashMap;

use uuid::Uuid;

use crate::avatar::AvatarAppearance;
use crate::chat::ChatType;
use crate::events::PresenceAddedEvent;
use crate::module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
use crate::registry::SceneRef;
pub use npc_avatar::NPCAvatar;

// ---------------------------------------------------------------------------
// NPCOptionsFlags
// ---------------------------------------------------------------------------

bitflags::bitflags! {
    /// Configuration flags controlling NPC behaviour, matching the C#
    /// `NPCOptionsFlags` enum.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct NPCOptionsFlags: u32 {
        const NONE                    = 0;
        /// Allow NPCs to exist without an owner (community NPCs).
        const ALLOW_NOT_OWNED         = 1;
        /// Allow NPCs to be sensed as real avatars by LSL sensors.
        const ALLOW_SENSE_AS_AVATAR   = 2;
        /// Allow cloning appearance from another avatar.
        const ALLOW_CLONE_OTHER_AVATARS = 4;
        /// Do not assign NPCs to the default NPC group.
        const NO_NPC_GROUP            = 8;
    }
}

// ---------------------------------------------------------------------------
// INPCModule trait
// ---------------------------------------------------------------------------

/// Interface for NPC management, matching the C# `INPCModule`.
pub trait INPCModule: Send + Sync {
    /// Create a new NPC in the scene.
    ///
    /// Returns the NPC's agent UUID on success.
    fn create_npc(
        &mut self,
        first_name: &str,
        last_name: &str,
        position: [f32; 3],
        owner_id: Uuid,
        sense_as_agent: bool,
        appearance: Option<AvatarAppearance>,
    ) -> Result<Uuid, ModuleError>;

    /// Move an NPC to a target position.
    fn move_to(&mut self, npc_id: Uuid, position: [f32; 3]) -> Result<(), ModuleError>;

    /// Make an NPC say a message on the given chat channel.
    fn say(&self, npc_id: Uuid, channel: i32, message: &str) -> Result<(), ModuleError>;

    /// Make an NPC sit on a target object.
    fn sit(&mut self, npc_id: Uuid, target: Uuid) -> Result<(), ModuleError>;

    /// Make an NPC stand up.
    fn stand(&mut self, npc_id: Uuid) -> Result<(), ModuleError>;

    /// Delete an NPC from the scene.
    fn delete_npc(&mut self, npc_id: Uuid, caller_id: Uuid) -> Result<(), ModuleError>;

    /// Set an NPC's rotation.
    fn rotate(&mut self, npc_id: Uuid, rotation: [f32; 4]) -> Result<(), ModuleError>;

    /// Make an NPC touch a target object (stub).
    fn touch(&self, npc_id: Uuid, target: Uuid) -> Result<(), ModuleError>;

    /// Get the owner of an NPC.
    fn get_owner(&self, npc_id: Uuid) -> Option<Uuid>;

    /// Get the current position of an NPC.
    fn get_npc_position(&self, npc_id: Uuid) -> Option<[f32; 3]>;

    /// Check whether a UUID belongs to an NPC.
    fn is_npc(&self, id: Uuid) -> bool;

    /// Get a reference to an NPC avatar.
    fn get_npc(&self, npc_id: Uuid) -> Option<&NPCAvatar>;
}

// ---------------------------------------------------------------------------
// NPCModule
// ---------------------------------------------------------------------------

/// Region module that manages NPC (non-player character) presences.
///
/// Mirrors the C# `NPCModule` from
/// `OpenSim/Region/OptionalModules/World/NPC/NPCModule.cs`.
pub struct NPCModule {
    /// Active NPC avatars keyed by agent UUID.
    avatars: HashMap<Uuid, NPCAvatar>,
    /// Maximum number of NPCs allowed in a single scene.
    pub max_npcs_per_scene: usize,
    /// Feature flags for NPC behaviour.
    pub option_flags: NPCOptionsFlags,
    /// Whether NPC persistence across region restarts is enabled.
    pub persistence_enabled: bool,
    /// Whether the module is enabled.
    enabled: bool,
    /// The scene this module is attached to.
    scene: Option<SceneRef>,
}

impl NPCModule {
    pub fn new() -> Self {
        Self {
            avatars: HashMap::new(),
            max_npcs_per_scene: 40,
            option_flags: NPCOptionsFlags::NONE,
            persistence_enabled: false,
            enabled: true,
            scene: None,
        }
    }

    /// Number of active NPCs.
    pub fn npc_count(&self) -> usize {
        self.avatars.len()
    }

    /// Validate that the caller has permission to manage this NPC.
    fn validate_owner(&self, npc_id: Uuid, caller_id: Uuid) -> Result<(), ModuleError> {
        let npc = self
            .avatars
            .get(&npc_id)
            .ok_or_else(|| ModuleError::NotFound(format!("NPC {npc_id} not found")))?;

        // If ALLOW_NOT_OWNED is set, anyone can manage unowned NPCs.
        if self.option_flags.contains(NPCOptionsFlags::ALLOW_NOT_OWNED) && npc.owner_id().is_nil() {
            return Ok(());
        }

        if npc.owner_id() != caller_id {
            return Err(ModuleError::Other(format!(
                "Agent {caller_id} is not the owner of NPC {npc_id}"
            )));
        }
        Ok(())
    }
}

impl Default for NPCModule {
    fn default() -> Self {
        Self::new()
    }
}

impl INPCModule for NPCModule {
    fn create_npc(
        &mut self,
        first_name: &str,
        last_name: &str,
        position: [f32; 3],
        owner_id: Uuid,
        sense_as_agent: bool,
        _appearance: Option<AvatarAppearance>,
    ) -> Result<Uuid, ModuleError> {
        if !self.enabled {
            return Err(ModuleError::InitFailed("NPCModule is not enabled".into()));
        }

        if self.avatars.len() >= self.max_npcs_per_scene {
            return Err(ModuleError::Other(format!(
                "Maximum NPC count ({}) reached",
                self.max_npcs_per_scene
            )));
        }

        // Check sense_as_agent permission.
        if sense_as_agent
            && !self
                .option_flags
                .contains(NPCOptionsFlags::ALLOW_SENSE_AS_AVATAR)
        {
            return Err(ModuleError::Other(
                "SenseAsAvatar is not allowed by module configuration".into(),
            ));
        }

        let npc_id = Uuid::new_v4();
        let npc = NPCAvatar::new(
            first_name,
            last_name,
            position,
            npc_id,
            owner_id,
            sense_as_agent,
            self.scene.clone(),
        );

        // Fire presence-added event.
        if let Some(ref scene) = self.scene {
            let evt = PresenceAddedEvent {
                region_id: scene.region_id,
                agent_id: npc_id,
                first_name: first_name.to_string(),
                last_name: last_name.to_string(),
            };
            let _ = scene.events.on_presence_added.send(evt);
        }

        self.avatars.insert(npc_id, npc);
        tracing::info!(npc = %npc_id, name = %format!("{first_name} {last_name}"), "NPC created");
        Ok(npc_id)
    }

    fn move_to(&mut self, npc_id: Uuid, position: [f32; 3]) -> Result<(), ModuleError> {
        let npc = self
            .avatars
            .get_mut(&npc_id)
            .ok_or_else(|| ModuleError::NotFound(format!("NPC {npc_id} not found")))?;
        npc.set_position(position);
        Ok(())
    }

    fn say(&self, npc_id: Uuid, channel: i32, message: &str) -> Result<(), ModuleError> {
        let npc = self
            .avatars
            .get(&npc_id)
            .ok_or_else(|| ModuleError::NotFound(format!("NPC {npc_id} not found")))?;
        if !npc.send_chat_message(message, ChatType::Say, channel) {
            return Err(ModuleError::Other("Failed to send chat message".into()));
        }
        Ok(())
    }

    fn sit(&mut self, npc_id: Uuid, target: Uuid) -> Result<(), ModuleError> {
        let npc = self
            .avatars
            .get_mut(&npc_id)
            .ok_or_else(|| ModuleError::NotFound(format!("NPC {npc_id} not found")))?;
        npc.sit(target);
        Ok(())
    }

    fn stand(&mut self, npc_id: Uuid) -> Result<(), ModuleError> {
        let npc = self
            .avatars
            .get_mut(&npc_id)
            .ok_or_else(|| ModuleError::NotFound(format!("NPC {npc_id} not found")))?;
        npc.stand();
        Ok(())
    }

    fn delete_npc(&mut self, npc_id: Uuid, caller_id: Uuid) -> Result<(), ModuleError> {
        self.validate_owner(npc_id, caller_id)?;

        if let Some(mut npc) = self.avatars.remove(&npc_id) {
            npc.disconnect();
            tracing::info!(npc = %npc_id, "NPC deleted");
            Ok(())
        } else {
            Err(ModuleError::NotFound(format!("NPC {npc_id} not found")))
        }
    }

    fn rotate(&mut self, npc_id: Uuid, rotation: [f32; 4]) -> Result<(), ModuleError> {
        let npc = self
            .avatars
            .get_mut(&npc_id)
            .ok_or_else(|| ModuleError::NotFound(format!("NPC {npc_id} not found")))?;
        npc.set_rotation(rotation);
        Ok(())
    }

    fn touch(&self, npc_id: Uuid, target: Uuid) -> Result<(), ModuleError> {
        let _npc = self
            .avatars
            .get(&npc_id)
            .ok_or_else(|| ModuleError::NotFound(format!("NPC {npc_id} not found")))?;
        // Stub: in a full implementation this would simulate a touch event
        // on the target prim.
        tracing::debug!(npc = %npc_id, target = %target, "NPC touch (stub)");
        Ok(())
    }

    fn get_owner(&self, npc_id: Uuid) -> Option<Uuid> {
        self.avatars.get(&npc_id).map(|npc| npc.owner_id())
    }

    fn get_npc_position(&self, npc_id: Uuid) -> Option<[f32; 3]> {
        self.avatars.get(&npc_id).map(|npc| npc.position())
    }

    fn is_npc(&self, id: Uuid) -> bool {
        self.avatars.contains_key(&id)
    }

    fn get_npc(&self, npc_id: Uuid) -> Option<&NPCAvatar> {
        self.avatars.get(&npc_id)
    }
}

impl RegionModule for NPCModule {
    fn name(&self) -> &str {
        "NPCModule"
    }

    fn module_type(&self) -> ModuleType {
        ModuleType::NonShared
    }

    fn initialise(&mut self, config: &dyn ModuleConfig) -> Result<(), ModuleError> {
        self.enabled = config.get_bool("NPC", "Enabled", true);
        self.max_npcs_per_scene = config.get_int("NPC", "MaxNumberOfNPCs", 40) as usize;
        self.persistence_enabled = config.get_bool("NPC", "PersistenceEnabled", false);

        let allow_not_owned = config.get_bool("NPC", "AllowNotOwned", false);
        let allow_sense = config.get_bool("NPC", "AllowSenseAsAvatar", false);
        let allow_clone = config.get_bool("NPC", "AllowCloneOtherAvatars", false);
        let no_npc_group = config.get_bool("NPC", "NoNPCGroup", false);

        let mut flags = NPCOptionsFlags::NONE;
        if allow_not_owned {
            flags |= NPCOptionsFlags::ALLOW_NOT_OWNED;
        }
        if allow_sense {
            flags |= NPCOptionsFlags::ALLOW_SENSE_AS_AVATAR;
        }
        if allow_clone {
            flags |= NPCOptionsFlags::ALLOW_CLONE_OTHER_AVATARS;
        }
        if no_npc_group {
            flags |= NPCOptionsFlags::NO_NPC_GROUP;
        }
        self.option_flags = flags;

        tracing::info!(
            enabled = self.enabled,
            max_npcs = self.max_npcs_per_scene,
            flags = ?self.option_flags,
            "NPCModule initialised"
        );
        Ok(())
    }

    fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        self.scene = Some(scene.clone());
        tracing::info!(region = %scene.region_name, "NPCModule attached to region");
        Ok(())
    }

    fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
        // Disconnect all NPCs on region shutdown.
        let npc_ids: Vec<Uuid> = self.avatars.keys().copied().collect();
        for id in npc_ids {
            if let Some(mut npc) = self.avatars.remove(&id) {
                npc.disconnect();
            }
        }
        self.scene = None;
        tracing::info!(region = %scene.region_name, "NPCModule detached from region");
        Ok(())
    }

    fn close(&mut self) -> Result<(), ModuleError> {
        let npc_ids: Vec<Uuid> = self.avatars.keys().copied().collect();
        for id in npc_ids {
            if let Some(mut npc) = self.avatars.remove(&id) {
                npc.disconnect();
            }
        }
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
    use std::collections::HashMap;

    // -- Test config helper --------------------------------------------------

    struct TestConfig {
        strings: HashMap<(String, String), String>,
        ints: HashMap<(String, String), i64>,
        bools: HashMap<(String, String), bool>,
        floats: HashMap<(String, String), f64>,
    }

    impl TestConfig {
        fn new() -> Self {
            Self {
                strings: HashMap::new(),
                ints: HashMap::new(),
                bools: HashMap::new(),
                floats: HashMap::new(),
            }
        }

        fn set_bool(&mut self, section: &str, key: &str, value: bool) {
            self.bools
                .insert((section.to_string(), key.to_string()), value);
        }

        fn set_int(&mut self, section: &str, key: &str, value: i64) {
            self.ints
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
        fn get_float(&self, section: &str, key: &str, default: f64) -> f64 {
            self.floats
                .get(&(section.to_string(), key.to_string()))
                .copied()
                .unwrap_or(default)
        }
    }

    fn test_scene() -> SceneRef {
        SceneRef::new(Uuid::new_v4(), "TestRegion".to_string())
    }

    fn setup_module() -> NPCModule {
        let mut module = NPCModule::new();
        let config = TestConfig::new();
        module.initialise(&config).unwrap();
        let scene = test_scene();
        module.add_region(&scene).unwrap();
        module
    }

    // -- Test 1: create-move-say round trip ----------------------------------

    #[test]
    fn create_move_say_roundtrip() {
        let mut module = setup_module();
        let scene = module.scene.clone().unwrap();
        let mut rx = scene.events.on_chat_message.subscribe();

        let owner = Uuid::new_v4();
        let npc_id = module
            .create_npc("Test", "NPC", [128.0, 128.0, 25.0], owner, false, None)
            .unwrap();

        // Verify creation.
        assert!(module.is_npc(npc_id));
        assert_eq!(module.get_owner(npc_id), Some(owner));

        // Move.
        module.move_to(npc_id, [64.0, 64.0, 30.0]).unwrap();
        assert_eq!(module.get_npc_position(npc_id), Some([64.0, 64.0, 30.0]));

        // Say.
        module.say(npc_id, 0, "Hello world").unwrap();
        let msg = rx.try_recv().unwrap();
        assert_eq!(msg.message, "Hello world");
        assert_eq!(msg.sender_id, npc_id);
    }

    // -- Test 2: appearance cloning (stub — verifies create accepts it) -------

    #[test]
    fn appearance_cloning() {
        let mut module = setup_module();
        let appearance = AvatarAppearance {
            serial: 1,
            visual_params: vec![0u8; 218],
            texture_entry: vec![],
            wearables: HashMap::new(),
            height: 1.9,
            hover_z: 0.0,
        };
        let owner = Uuid::new_v4();
        let npc_id = module
            .create_npc(
                "Clone",
                "Test",
                [0.0, 0.0, 0.0],
                owner,
                false,
                Some(appearance),
            )
            .unwrap();
        assert!(module.is_npc(npc_id));
        let npc = module.get_npc(npc_id).unwrap();
        assert_eq!(npc.first_name(), "Clone");
        assert_eq!(npc.last_name(), "Test");
    }

    // -- Test 3: osNpc* integration stubs (trait method coverage) -------------

    #[test]
    fn osnpc_integration_stubs() {
        let mut module = setup_module();
        let owner = Uuid::new_v4();
        let target_obj = Uuid::new_v4();

        let npc_id = module
            .create_npc("Stub", "NPC", [10.0, 20.0, 30.0], owner, false, None)
            .unwrap();

        // Sit / stand cycle.
        module.sit(npc_id, target_obj).unwrap();
        assert!(module.get_npc(npc_id).unwrap().is_sitting());
        module.stand(npc_id).unwrap();
        assert!(!module.get_npc(npc_id).unwrap().is_sitting());

        // Rotate.
        module.rotate(npc_id, [0.0, 0.707, 0.0, 0.707]).unwrap();
        assert_eq!(
            module.get_npc(npc_id).unwrap().rotation(),
            [0.0, 0.707, 0.0, 0.707]
        );

        // Touch (stub).
        module.touch(npc_id, target_obj).unwrap();
    }

    // -- Test 4: cleanup on region shutdown -----------------------------------

    #[test]
    fn cleanup_on_region_shutdown() {
        let mut module = setup_module();
        let scene = module.scene.clone().unwrap();
        let mut rx = scene.events.on_presence_removed.subscribe();
        let owner = Uuid::new_v4();

        let npc1 = module
            .create_npc("NPC", "One", [0.0, 0.0, 0.0], owner, false, None)
            .unwrap();
        let npc2 = module
            .create_npc("NPC", "Two", [10.0, 0.0, 0.0], owner, false, None)
            .unwrap();
        assert_eq!(module.npc_count(), 2);

        // Remove region should disconnect all NPCs.
        module.remove_region(&scene).unwrap();
        assert_eq!(module.npc_count(), 0);

        // Should have received presence-removed events.
        let mut removed_ids = Vec::new();
        while let Ok(evt) = rx.try_recv() {
            removed_ids.push(evt.agent_id);
        }
        assert!(removed_ids.contains(&npc1));
        assert!(removed_ids.contains(&npc2));
    }

    // -- Test 5: permission checks (owner validation) -------------------------

    #[test]
    fn permission_checks_owner_validation() {
        let mut module = setup_module();
        let owner = Uuid::new_v4();
        let stranger = Uuid::new_v4();

        let npc_id = module
            .create_npc("Owned", "NPC", [0.0, 0.0, 0.0], owner, false, None)
            .unwrap();

        // Owner can delete.
        assert!(module.delete_npc(npc_id, owner).is_ok());

        // Recreate for the stranger test.
        let npc_id2 = module
            .create_npc("Owned", "NPC2", [0.0, 0.0, 0.0], owner, false, None)
            .unwrap();

        // Stranger cannot delete.
        assert!(module.delete_npc(npc_id2, stranger).is_err());
        // NPC still exists.
        assert!(module.is_npc(npc_id2));
    }

    // -- Test 6: max NPC limit -----------------------------------------------

    #[test]
    fn max_npc_limit() {
        let mut module = setup_module();
        module.max_npcs_per_scene = 2;
        let owner = Uuid::new_v4();

        module
            .create_npc("A", "NPC", [0.0, 0.0, 0.0], owner, false, None)
            .unwrap();
        module
            .create_npc("B", "NPC", [0.0, 0.0, 0.0], owner, false, None)
            .unwrap();

        // Third should fail.
        let result = module.create_npc("C", "NPC", [0.0, 0.0, 0.0], owner, false, None);
        assert!(result.is_err());
        assert_eq!(module.npc_count(), 2);
    }

    // -- Test 7: sense_as_agent flag requires permission ----------------------

    #[test]
    fn sense_as_agent_requires_flag() {
        let mut module = setup_module();
        let owner = Uuid::new_v4();

        // Without ALLOW_SENSE_AS_AVATAR flag, creating with sense_as_agent=true
        // should fail.
        let result = module.create_npc("Sensor", "NPC", [0.0, 0.0, 0.0], owner, true, None);
        assert!(result.is_err());

        // Enable the flag.
        module.option_flags |= NPCOptionsFlags::ALLOW_SENSE_AS_AVATAR;
        let npc_id = module
            .create_npc("Sensor", "NPC", [0.0, 0.0, 0.0], owner, true, None)
            .unwrap();
        assert!(module.get_npc(npc_id).unwrap().sense_as_agent());
    }

    // -- Test 8: NPCAvatar name formatting -----------------------------------

    #[test]
    fn npc_avatar_name_formatting() {
        let npc = NPCAvatar::new(
            "John",
            "Doe",
            [0.0, 0.0, 0.0],
            Uuid::new_v4(),
            Uuid::new_v4(),
            false,
            None,
        );
        assert_eq!(npc.first_name(), "John");
        assert_eq!(npc.last_name(), "Doe");
        assert_eq!(npc.name(), "John Doe");
    }

    // -- Test 9: NPCAvatar disconnect fires event ----------------------------

    #[test]
    fn npc_avatar_disconnect_fires_event() {
        let scene = test_scene();
        let mut rx = scene.events.on_presence_removed.subscribe();
        let npc_id = Uuid::new_v4();

        let mut npc = NPCAvatar::new(
            "DC",
            "Test",
            [0.0, 0.0, 0.0],
            npc_id,
            Uuid::new_v4(),
            false,
            Some(scene.clone()),
        );

        assert!(!npc.is_disconnected());
        npc.disconnect();
        assert!(npc.is_disconnected());

        let evt = rx.try_recv().unwrap();
        assert_eq!(evt.agent_id, npc_id);

        // Double disconnect is a no-op.
        npc.disconnect();
    }

    // -- Test 10: NPCModule RegionModule lifecycle ----------------------------

    #[test]
    fn npc_module_lifecycle() {
        let mut module = NPCModule::new();
        let config = TestConfig::new();
        let scene = test_scene();

        module.initialise(&config).unwrap();
        module.add_region(&scene).unwrap();
        assert_eq!(module.name(), "NPCModule");
        assert_eq!(module.module_type(), ModuleType::NonShared);

        module.remove_region(&scene).unwrap();
        module.close().unwrap();
    }

    // -- Test 11: config initialisation reads values --------------------------

    #[test]
    fn config_initialisation() {
        let mut module = NPCModule::new();
        let mut config = TestConfig::new();
        config.set_bool("NPC", "Enabled", true);
        config.set_int("NPC", "MaxNumberOfNPCs", 100);
        config.set_bool("NPC", "AllowNotOwned", true);
        config.set_bool("NPC", "AllowSenseAsAvatar", true);
        config.set_bool("NPC", "PersistenceEnabled", true);

        module.initialise(&config).unwrap();

        assert!(module.enabled);
        assert_eq!(module.max_npcs_per_scene, 100);
        assert!(
            module
                .option_flags
                .contains(NPCOptionsFlags::ALLOW_NOT_OWNED)
        );
        assert!(
            module
                .option_flags
                .contains(NPCOptionsFlags::ALLOW_SENSE_AS_AVATAR)
        );
        assert!(module.persistence_enabled);
    }

    // -- Test 12: operations on nonexistent NPC return errors -----------------

    #[test]
    fn nonexistent_npc_errors() {
        let mut module = setup_module();
        let bogus = Uuid::new_v4();

        assert!(module.move_to(bogus, [0.0, 0.0, 0.0]).is_err());
        assert!(module.say(bogus, 0, "hello").is_err());
        assert!(module.sit(bogus, Uuid::new_v4()).is_err());
        assert!(module.stand(bogus).is_err());
        assert!(module.rotate(bogus, [0.0, 0.0, 0.0, 1.0]).is_err());
        assert!(module.touch(bogus, Uuid::new_v4()).is_err());
        assert!(module.delete_npc(bogus, Uuid::new_v4()).is_err());
        assert!(module.get_owner(bogus).is_none());
        assert!(module.get_npc_position(bogus).is_none());
        assert!(!module.is_npc(bogus));
        assert!(module.get_npc(bogus).is_none());
    }
}
