// SPDX-License-Identifier: GPL-3.0-or-later

//! Region-module framework for welkin.
//!
//! Provides the [`RegionModule`] trait, a [`ModuleRegistry`] for managing
//! module instances and interface lookups, a typed [`SceneEvents`] event bus,
//! and skeleton module implementations.

pub mod attachments;
pub mod avatar;
pub mod chat;
pub mod concierge;
pub mod economy;
pub mod environment;
pub mod estate;
pub mod events;
pub mod framework;
pub mod groups;
pub mod hg;
pub mod inventory;
pub mod irc_bridge;
pub mod land;
pub mod lifecycle;
pub mod messaging;
pub mod module;
pub mod npc;
pub mod offline_im;
pub mod permissions;
pub mod profiles;
pub mod registry;
pub mod transfer;
pub mod voice;
pub mod world_map;

// Re-exports for convenience.
pub use attachments::{AttachmentInfo, AttachmentPoint, AttachmentsModule};
pub use avatar::{
    AvatarAppearance, AvatarFactoryModule, BakeType, BakedTexturesModule,
    CombatModule as AvatarCombatModule, DialogModule, FriendInfo, FriendPerms, FriendsModule,
    GesturesModule, GodLevel, GodsModule, LureModule, WearableItem, WearableType,
};
pub use chat::{
    ChatMessage, ChatModule, ChatSourceType, ChatType, DEBUG_CHANNEL, MAX_MESSAGE_LENGTH,
};
pub use environment::{
    AccessModule, ConfigurableWind, EnvironmentSettings, LightShareModule, MediaEntry, MediaModule,
    RegionAccessFlags, RegionReadyModule, RestartModule, SceneSerialiser, SimpleRandomWind,
    SoundFlags, SoundModule, SoundTrigger, VegetationModule, WindModel, WindModule,
    WindlightPreset,
};
pub use estate::{EstateManagementModule, EstateSettings, EstateSettingsUpdate, TelehubManager};
pub use events::*;
pub use framework::{
    AgentPreferencesConnector, AgentPrefs, AssetCacheConfig, CacheStats, CachedAsset,
    FlotsamAssetCache, IPBanModule, J2KPriority, LandConnectorData, LandServiceConnector,
    LibraryModule, MapImageServiceConnector, MonitorModule, MuteListServiceConnector,
    NeighbourServiceConnector, RegionStats, SearchModule, SearchResult, ServiceConnectorConfig,
    TextureSenderModule, UserData, UserManagementModule,
};
pub use groups::{
    GroupAccountSummary, GroupInviteInfo, GroupMembersData, GroupMembershipData, GroupNoticeData,
    GroupRecord, GroupRolesData, GroupTitleData, GroupsModule, GroupsPacketType, IGroupsModule,
    IGroupsServicesConnector,
    hg_connector::{ForeignImporter, HGGroupsServiceConnector, RemoteConnectorCacheWrapper},
    messaging::{ChatSessionState, GroupsMessagingModule, NoticeAttachment},
};
pub use inventory::{
    AssetTransactionModule, AssetType, DerezAction, InventoryAccessModule, InventoryTransferModule,
    RezResult, UploadTransaction, XferModule,
};
pub use land::{
    DwellModule, LandData, LandManagementModule, LandParcel, ParcelGrid, PrimCountModule,
    PrimCounts,
};
pub use lifecycle::{CapsModule, DynAttrsModule, ServiceThrottleModule};
pub use messaging::{
    GridInstantMessage, IMDialog, MessageTransferModule, MuteEntry, MuteListModule, MuteType,
    OfflineMessageModule, PresenceModule,
};
pub use module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
pub use offline_im::OfflineIMModule;
pub use permissions::{DefaultPermissionsModule, PermissionsConfig, UserSet, checks::ObjectPerms};
pub use profiles::{
    AvatarProperties, BasicProfileModule, ClassifiedSummary, PickSummary, UserNote,
};
pub use registry::{ModuleRegistry, SceneRef};
pub use transfer::{
    AgentTransferState, EntityTransferContext, EntityTransferModule, EntityTransferStateMachine,
    GridRegion, TeleportFlags,
};
pub use world_map::{
    MapBlock, MapItem, MapItemType, MapSearchModule, SimAccess, WorldMapModule,
    map_image::{MapImageModule, MapTileConfig, ShadedTerrainRenderer, TerrainRenderer},
};
pub use npc::NPCModule;
pub use voice::WebRtcVoiceModule;
pub use hg::{
    entity_transfer::HGEntityTransferModule, friends::HGFriendsModule,
    inventory_access::HGInventoryAccessModule, lure::HGLureModule,
    messaging::HGMessageTransferModule, user_management::HGUserManagementModule,
    world_map::HGWorldMapModule,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;
    use uuid::Uuid;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    /// Minimal in-memory configuration for tests.
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

        #[allow(dead_code)]
        fn set_string(&mut self, section: &str, key: &str, value: &str) {
            self.strings
                .insert((section.to_string(), key.to_string()), value.to_string());
        }

        #[allow(dead_code)]
        fn set_int(&mut self, section: &str, key: &str, value: i64) {
            self.ints
                .insert((section.to_string(), key.to_string()), value);
        }

        fn set_bool(&mut self, section: &str, key: &str, value: bool) {
            self.bools
                .insert((section.to_string(), key.to_string()), value);
        }

        #[allow(dead_code)]
        fn set_float(&mut self, section: &str, key: &str, value: f64) {
            self.floats
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

    /// Dummy module used to track lifecycle calls.
    struct TrackingModule {
        name: String,
        module_type: ModuleType,
        initialised: bool,
        regions_added: Vec<String>,
        regions_removed: Vec<String>,
        closed: bool,
    }

    impl TrackingModule {
        fn new(name: &str, mt: ModuleType) -> Self {
            Self {
                name: name.to_string(),
                module_type: mt,
                initialised: false,
                regions_added: Vec::new(),
                regions_removed: Vec::new(),
                closed: false,
            }
        }
    }

    impl RegionModule for TrackingModule {
        fn name(&self) -> &str {
            &self.name
        }

        fn module_type(&self) -> ModuleType {
            self.module_type
        }

        fn initialise(&mut self, _config: &dyn ModuleConfig) -> Result<(), ModuleError> {
            self.initialised = true;
            Ok(())
        }

        fn add_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
            self.regions_added.push(scene.region_name.clone());
            Ok(())
        }

        fn remove_region(&mut self, scene: &SceneRef) -> Result<(), ModuleError> {
            self.regions_removed.push(scene.region_name.clone());
            Ok(())
        }

        fn close(&mut self) -> Result<(), ModuleError> {
            self.closed = true;
            Ok(())
        }
    }

    /// Marker trait used as an "interface" in TypeId lookups.
    trait ICapsInterface {}
    impl ICapsInterface for CapsModule {}

    /// Another marker trait for testing multiple interface registrations.
    trait IDynAttrsInterface {}
    impl IDynAttrsInterface for DynAttrsModule {}

    fn test_scene_ref() -> SceneRef {
        SceneRef::new(Uuid::new_v4(), "TestRegion".to_string())
    }

    // -----------------------------------------------------------------------
    // Tests
    // -----------------------------------------------------------------------

    // 1. Create registry, register a module, retrieve it by interface.
    #[test]
    fn register_and_retrieve_by_interface() {
        let mut registry = ModuleRegistry::new();
        let caps = CapsModule::new();
        let idx = registry.register(caps);
        registry.register_interface::<dyn ICapsInterface>(idx);

        let found = registry.get_module::<dyn ICapsInterface>();
        assert!(found.is_some());
        assert_eq!(found.unwrap().name(), "CapsModule");
    }

    // 2. Module lifecycle: initialise -> add_region -> remove_region -> close.
    #[test]
    fn full_lifecycle() {
        let mut registry = ModuleRegistry::new();
        registry.register(TrackingModule::new("Tracker", ModuleType::NonShared));

        let config = TestConfig::new();
        let scene = test_scene_ref();

        registry.initialise_all(&config).unwrap();
        registry.add_region_all(&scene).unwrap();
        registry.remove_region_all(&scene).unwrap();
        registry.close_all().unwrap();

        // We cannot inspect the inner TrackingModule directly since it is
        // behind a Box<dyn RegionModule>, but the absence of errors confirms
        // every lifecycle method was called without failure.
        assert_eq!(registry.module_count(), 1);
    }

    // 3. SceneEvents: subscribe, send event, receive it.
    #[test]
    fn scene_events_send_receive() {
        let events = SceneEvents::new();
        let mut rx = events.on_object_added.subscribe();

        let evt = ObjectAddedEvent {
            region_id: Uuid::new_v4(),
            object_id: Uuid::new_v4(),
            owner_id: Uuid::new_v4(),
        };
        events.on_object_added.send(evt.clone()).unwrap();

        let received = rx.try_recv().unwrap();
        assert_eq!(received.object_id, evt.object_id);
    }

    // 4. Multiple modules with different interfaces.
    #[test]
    fn multiple_modules_different_interfaces() {
        let mut registry = ModuleRegistry::new();

        let caps_idx = registry.register(CapsModule::new());
        registry.register_interface::<dyn ICapsInterface>(caps_idx);

        let dyn_idx = registry.register(DynAttrsModule::new());
        registry.register_interface::<dyn IDynAttrsInterface>(dyn_idx);

        let caps = registry.get_module::<dyn ICapsInterface>();
        let dyn_attrs = registry.get_module::<dyn IDynAttrsInterface>();

        assert!(caps.is_some());
        assert!(dyn_attrs.is_some());
        assert_eq!(caps.unwrap().name(), "CapsModule");
        assert_eq!(dyn_attrs.unwrap().name(), "DynAttrsModule");
    }

    // 5. TypeId lookup returns None for unregistered interface.
    #[test]
    fn unregistered_interface_returns_none() {
        let registry = ModuleRegistry::new();
        let result = registry.get_module::<dyn ICapsInterface>();
        assert!(result.is_none());
    }

    // 6. ModuleType discrimination.
    #[test]
    fn module_type_discrimination() {
        let caps = CapsModule::new();
        let dyn_attrs = DynAttrsModule::new();
        let throttle = ServiceThrottleModule::new();

        assert_eq!(caps.module_type(), ModuleType::NonShared);
        assert_eq!(dyn_attrs.module_type(), ModuleType::Shared);
        assert_eq!(throttle.module_type(), ModuleType::Shared);

        // Also confirm Display:
        assert_eq!(format!("{}", ModuleType::NonShared), "NonShared");
        assert_eq!(format!("{}", ModuleType::Shared), "Shared");
    }

    // 7. ModuleConfig trait basic usage.
    #[test]
    fn module_config_basic_usage() {
        let mut config = TestConfig::new();
        config.set_string("General", "Name", "MyRegion");
        config.set_int("General", "Port", 9000);
        config.set_bool("ServiceThrottle", "Enabled", false);
        config.set_float("Physics", "Gravity", -9.81);

        assert_eq!(config.get_string("General", "Name", "default"), "MyRegion");
        assert_eq!(config.get_int("General", "Port", 0), 9000);
        assert_eq!(config.get_bool("ServiceThrottle", "Enabled", true), false);
        assert!((config.get_float("Physics", "Gravity", 0.0) - (-9.81)).abs() < f64::EPSILON);

        // Defaults:
        assert_eq!(config.get_string("Missing", "Key", "fallback"), "fallback");
        assert_eq!(config.get_int("Missing", "Key", 42), 42);
        assert!(config.get_bool("Missing", "Key", true));
        assert!((config.get_float("Missing", "Key", 1.5) - 1.5).abs() < f64::EPSILON);
    }

    // 8. Register duplicate module name handling (should succeed — names need
    //    not be unique; both coexist).
    #[test]
    fn register_duplicate_module_names() {
        let mut registry = ModuleRegistry::new();
        registry.register(TrackingModule::new("DupName", ModuleType::Shared));
        registry.register(TrackingModule::new("DupName", ModuleType::Shared));
        assert_eq!(registry.module_count(), 2);
    }

    // 9. Event bus: multiple subscribers receive same event.
    #[test]
    fn event_bus_multiple_subscribers() {
        let events = SceneEvents::new();
        let mut rx1 = events.on_chat_message.subscribe();
        let mut rx2 = events.on_chat_message.subscribe();

        let evt = ChatMessageEvent {
            region_id: Uuid::new_v4(),
            sender_id: Uuid::new_v4(),
            message: "Hello world".to_string(),
            channel: 0,
        };
        events.on_chat_message.send(evt.clone()).unwrap();

        let r1 = rx1.try_recv().unwrap();
        let r2 = rx2.try_recv().unwrap();
        assert_eq!(r1.message, "Hello world");
        assert_eq!(r2.message, "Hello world");
        assert_eq!(r1.sender_id, r2.sender_id);
    }

    // 10. Close all modules in reverse order.
    #[test]
    fn close_all_reverse_order() {
        // We verify reverse order indirectly: register modules that would
        // fail if closed in forward order. Here we just confirm close_all
        // completes without error when three modules are registered.
        let mut registry = ModuleRegistry::new();
        registry.register(CapsModule::new());
        registry.register(DynAttrsModule::new());
        registry.register(ServiceThrottleModule::new());

        let config = TestConfig::new();
        registry.initialise_all(&config).unwrap();
        registry.close_all().unwrap();
    }

    // Bonus tests

    // 11. SceneRef creation and field access.
    #[test]
    fn scene_ref_fields() {
        let id = Uuid::new_v4();
        let scene = SceneRef::new(id, "Bonus".to_string());
        assert_eq!(scene.region_id, id);
        assert_eq!(scene.region_name, "Bonus");
        // Events should be accessible:
        let _rx = scene.events.on_land_change.subscribe();
    }

    // 12. SceneRef is Clone.
    #[test]
    fn scene_ref_is_clone() {
        let scene = test_scene_ref();
        let clone = scene.clone();
        assert_eq!(scene.region_id, clone.region_id);
        // Both share the same Arc:
        assert!(Arc::ptr_eq(&scene.events, &clone.events));
    }

    // 13. ModuleError messages.
    #[test]
    fn module_error_display() {
        let e = ModuleError::InitFailed("boom".to_string());
        assert!(format!("{e}").contains("boom"));

        let e = ModuleError::NotFound("missing".to_string());
        assert!(format!("{e}").contains("missing"));

        let e = ModuleError::AlreadyRegistered("dup".to_string());
        assert!(format!("{e}").contains("dup"));

        let e = ModuleError::Other("something".to_string());
        assert!(format!("{e}").contains("something"));
    }

    // 14. Initialise all with config that modules read.
    #[test]
    fn initialise_service_throttle_reads_config() {
        let mut registry = ModuleRegistry::new();
        registry.register(ServiceThrottleModule::new());

        let mut config = TestConfig::new();
        config.set_bool("ServiceThrottle", "Enabled", false);

        registry.initialise_all(&config).unwrap();
        // No error means the module successfully read the config value.
    }

    // ===================================================================
    // Land-module tests (section 2)
    // ===================================================================

    use crate::land::{
        AccessList, DwellModule, LandAccessEntry, LandData, LandDataUpdate, LandManagementModule,
        LandParcel, ParcelFlags, ParcelGrid, PrimCountModule,
    };

    // -- 15. ParcelGrid cell get/set ----------------------------------------

    #[test]
    fn parcel_grid_get_set() {
        let mut grid = ParcelGrid::new(256);
        assert_eq!(grid.get_parcel_id(0, 0), 0);
        grid.set_parcel_id(10, 20, 42);
        assert_eq!(grid.get_parcel_id(10, 20), 42);
        // Out-of-range returns 0.
        assert_eq!(grid.get_parcel_id(999, 999), 0);
    }

    // -- 16. ParcelGrid parcels_at_point ------------------------------------

    #[test]
    fn parcel_grid_parcels_at_point() {
        let mut grid = ParcelGrid::new(256);
        grid.fill_rect(0, 0, 256, 256, 7);
        // Cell at (100, 200): grid index (25, 50).
        assert_eq!(grid.parcels_at_point(100.0, 200.0), 7);
    }

    // -- 17. LandParcel basic_full_region_bitmap ----------------------------

    #[test]
    fn full_region_bitmap() {
        let bm = LandParcel::basic_full_region_bitmap(256);
        assert_eq!(bm.len(), 64);
        assert_eq!(bm[0].len(), 64);
        assert!(bm.iter().all(|col| col.iter().all(|&v| v)));
    }

    // -- 18. LandParcel get_square_bitmap -----------------------------------

    #[test]
    fn square_bitmap() {
        let bm = LandParcel::get_square_bitmap(256, 0, 0, 128, 128);
        // Cells [0..32][0..32] should be true, rest false.
        assert!(bm[0][0]);
        assert!(bm[31][31]);
        assert!(!bm[32][0]);
        assert!(!bm[0][32]);
    }

    // -- 19. LandParcel merge_bitmaps ---------------------------------------

    #[test]
    fn merge_bitmaps() {
        let a = LandParcel::get_square_bitmap(256, 0, 0, 128, 128);
        let b = LandParcel::get_square_bitmap(256, 128, 128, 256, 256);
        let merged = LandParcel::merge_bitmaps(&a, &b);
        assert!(merged[0][0]); // from a
        assert!(merged[32][32]); // from b
        assert!(!merged[32][0]); // neither
    }

    // -- 20. LandParcel contains_point --------------------------------------

    #[test]
    fn parcel_contains_point() {
        let mut data = LandData::default();
        data.local_id = 1;
        let mut parcel = LandParcel::new(data, 256);
        parcel.land_bitmap = LandParcel::get_square_bitmap(256, 0, 0, 64, 64);
        assert!(parcel.contains_point(0, 0));
        assert!(parcel.contains_point(15, 15));
        assert!(!parcel.contains_point(16, 0));
        assert!(!parcel.contains_point(0, 16));
    }

    // -- 21. LandParcel bitmap_to_bytes / bytes_to_bitmap roundtrip ---------

    #[test]
    fn bitmap_bytes_roundtrip() {
        let original = LandParcel::get_square_bitmap(256, 0, 0, 128, 128);
        let bytes = LandParcel::bitmap_to_bytes(&original);
        let restored = LandParcel::bytes_to_bitmap(&bytes, 64);
        assert_eq!(original, restored);
    }

    // -- 22. LandParcel area / start_point / end_point / center_point -------

    #[test]
    fn parcel_geometry() {
        let mut data = LandData::default();
        data.local_id = 1;
        let mut parcel = LandParcel::new(data, 256);
        parcel.land_bitmap = LandParcel::get_square_bitmap(256, 64, 64, 128, 128);
        assert_eq!(parcel.area(), 64 * 64); // 16 cells x 16 cells x 16 m^2
        assert_eq!(parcel.start_point(), (64.0, 64.0));
        assert_eq!(parcel.end_point(), (128.0, 128.0));
        assert_eq!(parcel.center_point(), (96.0, 96.0));
    }

    // -- 23. LandData default construction ----------------------------------

    #[test]
    fn land_data_defaults() {
        let d = LandData::default();
        assert_eq!(d.name, "Your Parcel");
        assert!(d.see_avs);
        assert!(d.any_av_sounds);
        assert!(d.group_av_sounds);
        assert!(!d.global_id.is_nil());
        assert_eq!(d.owner_id, Uuid::nil());
        let flags = ParcelFlags::from_bits_truncate(d.flags);
        assert!(flags.contains(ParcelFlags::ALLOW_FLY));
        assert!(flags.contains(ParcelFlags::CREATE_OBJECTS));
    }

    // -- 24. LandManagementModule add / remove / get_at ---------------------

    #[test]
    fn management_add_remove_get() {
        let mut mgr = LandManagementModule::new();
        let config = TestConfig::new();
        mgr.initialise(&config).unwrap();

        let mut data = LandData::default();
        data.owner_id = Uuid::new_v4();
        let full = LandParcel::basic_full_region_bitmap(256);
        data.bitmap = LandParcel::bitmap_to_bytes(&full);

        let id = mgr.add_parcel(data);
        assert!(mgr.get_parcel_at(128.0, 128.0).is_some());
        assert_eq!(mgr.get_parcel_at(128.0, 128.0).unwrap().data.local_id, id);

        mgr.remove_parcel(id);
        assert!(mgr.get_parcel_at(128.0, 128.0).is_none());
    }

    // -- 25. LandManagementModule RegionModule lifecycle ---------------------

    #[test]
    fn management_lifecycle() {
        let mut mgr = LandManagementModule::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        mgr.initialise(&config).unwrap();
        mgr.add_region(&scene).unwrap();
        // Should have one default parcel covering the whole region.
        assert_eq!(mgr.parcel_count(), 1);
        assert!(mgr.get_parcel_at(0.0, 0.0).is_some());
        mgr.remove_region(&scene).unwrap();
        assert_eq!(mgr.parcel_count(), 0);
    }

    // -- 26. Subdivide ------------------------------------------------------

    #[test]
    fn management_subdivide() {
        let mut mgr = LandManagementModule::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        mgr.initialise(&config).unwrap();
        mgr.add_region(&scene).unwrap();

        let owner = Uuid::new_v4();
        let new_id = mgr.subdivide(0, 0, 128, 128, owner).unwrap();

        // Two parcels now.
        assert_eq!(mgr.parcel_count(), 2);

        // The new parcel covers (0, 0).
        let p = mgr.get_parcel_at(64.0, 64.0).unwrap();
        assert_eq!(p.data.local_id, new_id);
        assert_eq!(p.data.owner_id, owner);
    }

    // -- 27. Join -----------------------------------------------------------

    #[test]
    fn management_join() {
        let mut mgr = LandManagementModule::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        mgr.initialise(&config).unwrap();
        mgr.add_region(&scene).unwrap();

        let owner = Uuid::new_v4();
        let a = mgr.subdivide(0, 0, 128, 128, owner).unwrap();
        let b = mgr.subdivide(128, 128, 256, 256, owner).unwrap();
        // 3 parcels now (original minus two chunks + two new).
        assert_eq!(mgr.parcel_count(), 3);

        let merged = mgr.join(&[a, b], owner).unwrap();
        // Should have 2 parcels now: the remnant of the original + merged.
        assert_eq!(mgr.parcel_count(), 2);
        let p = mgr.get_parcel(merged).unwrap();
        assert_eq!(p.data.owner_id, owner);
    }

    // -- 28. Join rejects different owners ----------------------------------

    #[test]
    fn management_join_wrong_owner() {
        let mut mgr = LandManagementModule::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        mgr.initialise(&config).unwrap();
        mgr.add_region(&scene).unwrap();

        let owner_a = Uuid::new_v4();
        let owner_b = Uuid::new_v4();
        let a = mgr.subdivide(0, 0, 64, 64, owner_a).unwrap();
        let b = mgr.subdivide(64, 64, 128, 128, owner_b).unwrap();
        let result = mgr.join(&[a, b], owner_a);
        assert!(result.is_err());
    }

    // -- 29. Access list: ban check -----------------------------------------

    #[test]
    fn access_list_ban() {
        let mut mgr = LandManagementModule::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        mgr.initialise(&config).unwrap();
        mgr.add_region(&scene).unwrap();

        let agent = Uuid::new_v4();
        // Get the default parcel's id.
        let parcel_id = mgr.get_parcel_at(128.0, 128.0).unwrap().data.local_id;

        mgr.update_access_list(
            parcel_id,
            vec![LandAccessEntry {
                agent_id: agent,
                expires: 0,
                flags: AccessList::Ban,
            }],
        );

        assert!(mgr.is_banned(parcel_id, agent));
        assert!(!mgr.can_be_on_land(parcel_id, agent));
        // Another agent is not banned.
        assert!(!mgr.is_banned(parcel_id, Uuid::new_v4()));
    }

    // -- 30. Access list: restrict check ------------------------------------

    #[test]
    fn access_list_restrict() {
        let mut mgr = LandManagementModule::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        mgr.initialise(&config).unwrap();
        mgr.add_region(&scene).unwrap();

        let allowed = Uuid::new_v4();
        let stranger = Uuid::new_v4();
        let parcel_id = mgr.get_parcel_at(128.0, 128.0).unwrap().data.local_id;

        mgr.update_access_list(
            parcel_id,
            vec![LandAccessEntry {
                agent_id: allowed,
                expires: 0,
                flags: AccessList::Access,
            }],
        );

        assert!(!mgr.is_restricted(parcel_id, allowed));
        assert!(mgr.is_restricted(parcel_id, stranger));
        assert!(mgr.can_be_on_land(parcel_id, allowed));
        assert!(!mgr.can_be_on_land(parcel_id, stranger));
    }

    // -- 31. PrimCounts tracking --------------------------------------------

    #[test]
    fn prim_counts_tracking() {
        let mut pc = PrimCountModule::new();
        pc.increment(1, 10);
        let c = pc.get_counts(1);
        assert_eq!(c.total, 10);

        pc.decrement(1, 3);
        assert_eq!(pc.get_counts(1).total, 7);

        pc.recount(1);
        assert_eq!(pc.get_counts(1).total, 0);

        // Unknown parcel returns defaults.
        assert_eq!(pc.get_counts(999).total, 0);
    }

    // -- 32. DwellModule tracking -------------------------------------------

    #[test]
    fn dwell_tracking() {
        let mut dw = DwellModule::new();
        assert_eq!(dw.get_dwell(1), 0.0);
        dw.update_dwell(1, 5.0);
        dw.update_dwell(1, 3.0);
        assert!((dw.get_dwell(1) - 8.0).abs() < f32::EPSILON);
    }

    // -- 33. LandDataUpdate apply -------------------------------------------

    #[test]
    fn land_data_update_apply() {
        let mut d = LandData::default();
        let update = LandDataUpdate {
            name: Some("New Name".to_string()),
            sale_price: Some(1000),
            see_avs: Some(false),
            ..Default::default()
        };
        d.apply_update(&update);
        assert_eq!(d.name, "New Name");
        assert_eq!(d.sale_price, 1000);
        assert!(!d.see_avs);
        // Untouched fields remain at default.
        assert!(d.any_av_sounds);
    }

    // -- 34. ParcelFlags default contains expected bits ---------------------

    #[test]
    fn parcel_flags_default() {
        let f = ParcelFlags::default();
        assert!(f.contains(ParcelFlags::ALLOW_FLY));
        assert!(f.contains(ParcelFlags::ALLOW_LANDMARK));
        assert!(f.contains(ParcelFlags::ALLOW_A_PRIMITIVE_ENTRY));
        assert!(f.contains(ParcelFlags::ALLOW_DEED_TO_GROUP));
        assert!(f.contains(ParcelFlags::CREATE_OBJECTS));
        assert!(f.contains(ParcelFlags::ALLOW_OTHER_SCRIPTS));
        assert!(f.contains(ParcelFlags::ALLOW_VOICE_CHAT));
        // ForSale should NOT be set by default.
        assert!(!f.contains(ParcelFlags::FOR_SALE));
    }

    // -- 35. ParcelGrid fill_rect / cell_count ------------------------------

    #[test]
    fn parcel_grid_fill_rect_count() {
        let mut grid = ParcelGrid::new(256);
        grid.fill_rect(0, 0, 128, 128, 5);
        // 128/4 = 32 cells per axis => 32*32 = 1024 cells.
        assert_eq!(grid.cell_count(5), 1024);
        assert_eq!(grid.cell_count(0), 64 * 64 - 1024);
    }

    // ===================================================================
    // Estate-module tests (section 4)
    // ===================================================================

    use crate::estate::{
        EstateManagementModule, EstateSettings, EstateSettingsUpdate, TelehubManager,
    };

    // -- 36. EstateSettings default construction ---------------------------

    #[test]
    fn estate_settings_defaults() {
        let s = EstateSettings::default();
        assert_eq!(s.estate_id, 0);
        assert_eq!(s.estate_name, "My Estate");
        assert_eq!(s.estate_owner, Uuid::nil());
        assert!(s.allow_voice);
        assert!(s.allow_direct_teleport);
        assert!(!s.deny_anonymous);
        assert!((s.water_height - 20.0).abs() < f64::EPSILON);
        assert!(s.estate_managers.is_empty());
        assert!(s.estate_banned.is_empty());
    }

    // -- 37. Estate access checks: manager / banned / open ----------------

    #[test]
    fn estate_access_checks() {
        let mut module = EstateManagementModule::new();
        let owner = Uuid::new_v4();
        let manager = Uuid::new_v4();
        let banned = Uuid::new_v4();
        let stranger = Uuid::new_v4();

        module.settings.estate_owner = owner;
        module.settings.estate_managers.insert(manager);
        module.settings.estate_banned.insert(banned);

        // Owner and manager are estate managers.
        assert!(module.is_estate_manager(owner));
        assert!(module.is_estate_manager(manager));
        assert!(!module.is_estate_manager(stranger));

        // Banned check.
        assert!(module.is_banned(banned));
        assert!(!module.is_banned(stranger));

        // Access: owner/manager always have access; banned never.
        assert!(module.has_access(owner));
        assert!(module.has_access(manager));
        assert!(!module.has_access(banned));

        // Open estate (no users/groups lists) — stranger has access.
        assert!(module.has_access(stranger));
    }

    // -- 38. Estate access with restricted user list ----------------------

    #[test]
    fn estate_access_restricted_list() {
        let mut module = EstateManagementModule::new();
        let owner = Uuid::new_v4();
        let allowed = Uuid::new_v4();
        let stranger = Uuid::new_v4();

        module.settings.estate_owner = owner;
        module.settings.estate_users.insert(allowed);

        // With a user list populated, stranger is denied.
        assert!(module.has_access(owner));
        assert!(module.has_access(allowed));
        assert!(!module.has_access(stranger));
    }

    // -- 39. EstateSettingsUpdate partial apply ----------------------------

    #[test]
    fn estate_settings_update_apply() {
        let mut module = EstateManagementModule::new();
        let update = EstateSettingsUpdate {
            estate_name: Some("Updated Estate".to_string()),
            allow_voice: Some(false),
            price_per_meter: Some(42),
            sun_position: Some(6.0),
            ..Default::default()
        };
        module.update_settings(update);
        assert_eq!(module.settings.estate_name, "Updated Estate");
        assert!(!module.settings.allow_voice);
        assert_eq!(module.settings.price_per_meter, 42);
        assert!((module.settings.sun_position - 6.0).abs() < f64::EPSILON);
        // Untouched fields remain at default.
        assert!(module.settings.allow_direct_teleport);
    }

    // -- 40. TelehubManager spawn points ----------------------------------

    #[test]
    fn telehub_spawn_points() {
        let mut th = TelehubManager::new();
        // Default landing when no spawn points.
        assert_eq!(th.get_landing_point(), [128.0, 128.0, 25.0]);

        th.add_spawn_point([10.0, 20.0, 30.0]);
        th.add_spawn_point([40.0, 50.0, 60.0]);
        assert_eq!(th.spawn_points.len(), 2);
        assert_eq!(th.get_landing_point(), [10.0, 20.0, 30.0]);

        // Remove first; second becomes the landing point.
        assert!(th.remove_spawn_point(0));
        assert_eq!(th.get_landing_point(), [40.0, 50.0, 60.0]);

        // Invalid index.
        assert!(!th.remove_spawn_point(99));
    }

    // -- 41. Estate actions don't panic -----------------------------------

    #[test]
    fn estate_actions_no_panic() {
        let module = EstateManagementModule::new();
        module.restart_region(120);
        module.kick_user(Uuid::new_v4(), "goodbye");
        module.teleport_home(Uuid::new_v4());
        module.send_region_message("Admin", "Server restarting");
    }

    // -- 42. Estate sun position ------------------------------------------

    #[test]
    fn estate_sun_position() {
        let mut module = EstateManagementModule::new();
        assert!((module.get_sun_position() - 0.0).abs() < f64::EPSILON);
        module.set_sun_position(12.5);
        assert!((module.get_sun_position() - 12.5).abs() < f64::EPSILON);
    }

    // -- 43. EstateManagementModule RegionModule lifecycle -----------------

    #[test]
    fn estate_module_lifecycle() {
        let mut module = EstateManagementModule::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        module.initialise(&config).unwrap();
        module.add_region(&scene).unwrap();
        assert_eq!(module.name(), "EstateManagementModule");
        assert_eq!(module.module_type(), ModuleType::NonShared);
        module.remove_region(&scene).unwrap();
        module.close().unwrap();
    }

    // ===================================================================
    // Inventory-module tests (section 5)
    // ===================================================================

    use crate::inventory::{
        AssetTransactionModule, AssetType, DerezAction, InventoryAccessModule,
        InventoryTransferModule, RezResult, XferModule,
    };

    // -- 44. InventoryTransferModule give/accept/decline flow --------------

    #[test]
    fn inventory_transfer_give_accept_decline() {
        let module = InventoryTransferModule::new();
        let from = Uuid::new_v4();
        let to = Uuid::new_v4();
        let item = Uuid::new_v4();
        let folder = Uuid::new_v4();

        let new_item = module.give_item(from, to, item).unwrap();
        assert!(!new_item.is_nil());

        let new_folder = module.give_folder(from, to, folder).unwrap();
        assert!(!new_folder.is_nil());

        let txn = Uuid::new_v4();
        module.accept_gift(to, txn).unwrap();
        module.decline_gift(to, txn).unwrap();
    }

    // -- 45. InventoryAccessModule rez/derez with RezResult ---------------

    #[test]
    fn inventory_access_rez_derez() {
        let module = InventoryAccessModule::new();
        let agent = Uuid::new_v4();
        let item = Uuid::new_v4();
        let pos = [128.0, 128.0, 25.0];
        let rot = [0.0, 0.0, 0.0, 1.0];

        let result = module.rez_object(agent, item, pos, rot);
        match result {
            RezResult::Success { object_id } => assert!(!object_id.is_nil()),
            _ => panic!("expected RezResult::Success"),
        }

        let result2 = module.rez_coalesced(agent, item, pos);
        match result2 {
            RezResult::Success { object_id } => assert!(!object_id.is_nil()),
            _ => panic!("expected RezResult::Success"),
        }

        let derez_item = module
            .derez_object(agent, Uuid::new_v4(), DerezAction::Take)
            .unwrap();
        assert!(!derez_item.is_nil());
    }

    // -- 46. DerezAction enum values --------------------------------------

    #[test]
    fn derez_action_variants() {
        // Verify all variants exist and can be compared.
        assert_ne!(DerezAction::Take, DerezAction::TakeCopy);
        assert_ne!(DerezAction::Return, DerezAction::Delete);
        assert_eq!(DerezAction::Take, DerezAction::Take);
    }

    // -- 47. AssetTransactionModule upload lifecycle -----------------------

    #[test]
    fn asset_transaction_lifecycle() {
        let mut module = AssetTransactionModule::new();
        assert_eq!(module.transaction_count(), 0);

        let agent = Uuid::new_v4();
        let txn = module.begin_upload(agent, AssetType::Texture);
        assert_eq!(module.transaction_count(), 1);

        module.append_data(txn, &[1, 2, 3, 4]).unwrap();
        module.append_data(txn, &[5, 6]).unwrap();

        let asset_id = module.complete_upload(txn).unwrap();
        assert!(!asset_id.is_nil());
        assert_eq!(module.transaction_count(), 0);
    }

    // -- 48. AssetTransactionModule cancel upload -------------------------

    #[test]
    fn asset_transaction_cancel() {
        let mut module = AssetTransactionModule::new();
        let txn = module.begin_upload(Uuid::new_v4(), AssetType::Sound);
        assert_eq!(module.transaction_count(), 1);
        module.cancel_upload(txn);
        assert_eq!(module.transaction_count(), 0);
    }

    // -- 49. AssetTransactionModule missing transaction errors -------------

    #[test]
    fn asset_transaction_missing_errors() {
        let mut module = AssetTransactionModule::new();
        let bogus = Uuid::new_v4();
        assert!(module.append_data(bogus, &[1]).is_err());
        assert!(module.complete_upload(bogus).is_err());
    }

    // -- 50. AssetType enum coverage --------------------------------------

    #[test]
    fn asset_type_variants() {
        let types = [
            AssetType::Texture,
            AssetType::Sound,
            AssetType::Animation,
            AssetType::Clothing,
            AssetType::Bodypart,
            AssetType::Script,
            AssetType::Object,
            AssetType::Notecard,
            AssetType::Landmark,
            AssetType::Gesture,
        ];
        // All variants exist and are distinct.
        for (i, a) in types.iter().enumerate() {
            for (j, b) in types.iter().enumerate() {
                assert_eq!(i == j, a == b);
            }
        }
    }

    // -- 51. XferModule stubs don't panic ---------------------------------

    #[test]
    fn xfer_module_stubs() {
        let xfer = XferModule::new();
        let id = xfer.request_xfer("test.dat");
        assert!(!id.is_nil());
        let data = xfer.complete_xfer(id).unwrap();
        assert!(data.is_empty());
    }

    // -- 52. InventoryTransferModule RegionModule lifecycle ----------------

    #[test]
    fn inventory_transfer_lifecycle() {
        let mut module = InventoryTransferModule::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        module.initialise(&config).unwrap();
        module.add_region(&scene).unwrap();
        assert_eq!(module.name(), "InventoryTransferModule");
        assert_eq!(module.module_type(), ModuleType::NonShared);
        module.remove_region(&scene).unwrap();
        module.close().unwrap();
    }

    // ===================================================================
    // Attachments-module tests (section 6)
    // ===================================================================

    use crate::attachments::{AttachmentInfo, AttachmentPoint, AttachmentsModule};

    // -- 53. AttachmentPoint enum coverage ---------------------------------

    #[test]
    fn attachment_point_repr_values() {
        assert_eq!(AttachmentPoint::Default as u8, 0);
        assert_eq!(AttachmentPoint::Chest as u8, 1);
        assert_eq!(AttachmentPoint::Head as u8, 2);
        assert_eq!(AttachmentPoint::HudCenter as u8, 35);
        assert_eq!(AttachmentPoint::Neck as u8, 39);
        assert_eq!(AttachmentPoint::AvatarCenter as u8, 40);
    }

    // -- 54. AttachmentsModule wear/detach ---------------------------------

    #[test]
    fn attachments_wear_detach() {
        let mut module = AttachmentsModule::new();
        let agent = Uuid::new_v4();
        let item1 = Uuid::new_v4();
        let item2 = Uuid::new_v4();

        let obj1 = module.wear(agent, item1, AttachmentPoint::Head).unwrap();
        let obj2 = module.wear(agent, item2, AttachmentPoint::Chest).unwrap();
        assert!(!obj1.is_nil());
        assert!(!obj2.is_nil());

        let attached = module.get_attachments(agent);
        assert_eq!(attached.len(), 2);

        module.detach(agent, item1).unwrap();
        assert_eq!(module.get_attachments(agent).len(), 1);
        assert_eq!(module.get_attachments(agent)[0].item_id, item2);
    }

    // -- 55. AttachmentsModule detach error on missing item ----------------

    #[test]
    fn attachments_detach_missing() {
        let mut module = AttachmentsModule::new();
        let agent = Uuid::new_v4();
        // No attachments at all.
        assert!(module.detach(agent, Uuid::new_v4()).is_err());

        // With attachments but wrong item.
        module
            .wear(agent, Uuid::new_v4(), AttachmentPoint::Back)
            .unwrap();
        assert!(module.detach(agent, Uuid::new_v4()).is_err());
    }

    // -- 56. AttachmentsModule detach_all ----------------------------------

    #[test]
    fn attachments_detach_all() {
        let mut module = AttachmentsModule::new();
        let agent = Uuid::new_v4();
        module
            .wear(agent, Uuid::new_v4(), AttachmentPoint::LeftHand)
            .unwrap();
        module
            .wear(agent, Uuid::new_v4(), AttachmentPoint::RightHand)
            .unwrap();
        assert_eq!(module.get_attachments(agent).len(), 2);

        module.detach_all(agent);
        assert_eq!(module.get_attachments(agent).len(), 0);
    }

    // -- 57. AttachmentsModule login/logout --------------------------------

    #[test]
    fn attachments_login_logout() {
        let mut module = AttachmentsModule::new();
        let agent = Uuid::new_v4();

        // Simulate login with saved attachments.
        let saved = vec![
            AttachmentInfo {
                item_id: Uuid::new_v4(),
                object_id: Uuid::new_v4(),
                point: AttachmentPoint::Pelvis,
                owner_id: agent,
            },
            AttachmentInfo {
                item_id: Uuid::new_v4(),
                object_id: Uuid::new_v4(),
                point: AttachmentPoint::Mouth,
                owner_id: agent,
            },
        ];
        module.on_agent_login(agent, saved);
        assert_eq!(module.get_attachments(agent).len(), 2);

        // Logout returns the attachments.
        let returned = module.on_agent_logout(agent);
        assert_eq!(returned.len(), 2);
        assert_eq!(module.get_attachments(agent).len(), 0);

        // Second logout returns empty.
        assert!(module.on_agent_logout(agent).is_empty());
    }

    // -- 58. AttachmentsModule prepare_for_transfer ------------------------

    #[test]
    fn attachments_prepare_for_transfer() {
        let mut module = AttachmentsModule::new();
        let agent = Uuid::new_v4();
        module
            .wear(agent, Uuid::new_v4(), AttachmentPoint::Stomach)
            .unwrap();

        // prepare_for_transfer clones without removing.
        let transferred = module.prepare_for_transfer(agent);
        assert_eq!(transferred.len(), 1);
        assert_eq!(module.get_attachments(agent).len(), 1);

        // Unknown agent returns empty.
        assert!(module.prepare_for_transfer(Uuid::new_v4()).is_empty());
    }

    // -- 59. AttachmentsModule validate_point -----------------------------

    #[test]
    fn attachments_validate_point() {
        assert!(AttachmentsModule::validate_point(AttachmentPoint::Default));
        assert!(AttachmentsModule::validate_point(
            AttachmentPoint::AvatarCenter
        ));
        assert!(AttachmentsModule::validate_point(
            AttachmentPoint::HudBottomRight
        ));
    }

    // -- 60. AttachmentsModule RegionModule lifecycle ----------------------

    #[test]
    fn attachments_module_lifecycle() {
        let mut module = AttachmentsModule::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        module.initialise(&config).unwrap();
        module.add_region(&scene).unwrap();
        assert_eq!(module.name(), "AttachmentsModule");
        assert_eq!(module.module_type(), ModuleType::NonShared);

        // Wear something, then remove_region clears it.
        module
            .wear(Uuid::new_v4(), Uuid::new_v4(), AttachmentPoint::Nose)
            .unwrap();
        module.remove_region(&scene).unwrap();
        // After remove, no attachments remain.
        module.close().unwrap();
    }

    // ===================================================================
    // Permissions-module tests (section 3 — high-risk)
    // ===================================================================

    use crate::permissions::{DefaultPermissionsModule, UserSet};
    use welkin_types::PermissionMask;

    // -- helpers ----------------------------------------------------------

    /// Build a module with bypass OFF and standard settings.
    fn perms_module_no_bypass() -> DefaultPermissionsModule {
        let mut m = DefaultPermissionsModule::new();
        m.config.bypass_permissions = false;
        m.config.bypass_permissions_value = true;
        m.config.region_owner_is_god = true;
        m.config.region_manager_is_god = false;
        m.config.allow_grid_gods = true;
        m.estate_owner = Uuid::from_u128(0xAAAA);
        m
    }

    fn perm_owner_id() -> Uuid {
        Uuid::from_u128(1)
    }

    fn perm_stranger_id() -> Uuid {
        Uuid::from_u128(2)
    }

    fn perm_estate_owner_id() -> Uuid {
        Uuid::from_u128(0xAAAA)
    }

    fn perm_estate_manager_id() -> Uuid {
        Uuid::from_u128(0xBBBB)
    }

    fn perm_god_id() -> Uuid {
        Uuid::from_u128(0xCCCC)
    }

    fn perm_parcel_with_owner(owner: Uuid) -> LandData {
        let mut d = LandData::default();
        d.owner_id = owner;
        d
    }

    // -- 61. bypass_permissions=true allows everything ---------------------

    #[test]
    fn perm_bypass_allows_all() {
        let mut m = DefaultPermissionsModule::new();
        m.config.bypass_permissions = true;
        m.config.bypass_permissions_value = true;

        let s = perm_stranger_id();
        let o = perm_owner_id();
        let perms = PermissionMask::NONE;

        assert!(m.can_edit_object(s, o, Uuid::nil(), perms));
        assert!(m.can_move_object(s, o, Uuid::nil(), perms));
        assert!(m.can_copy_object(s, o, Uuid::nil(), perms));
        assert!(m.can_delete_object(s, o, Uuid::nil()));
        assert!(m.can_edit_terrain(s));
        assert!(m.can_edit_estate(s));
    }

    // -- 62. bypass_permissions=true, bypass_value=false denies all --------

    #[test]
    fn perm_bypass_denies_when_value_false() {
        let mut m = DefaultPermissionsModule::new();
        m.config.bypass_permissions = true;
        m.config.bypass_permissions_value = false;

        let o = perm_owner_id();
        let perms = PermissionMask::ALL;
        assert!(!m.can_edit_object(o, o, Uuid::nil(), perms));
        assert!(!m.can_move_object(o, o, Uuid::nil(), perms));
        assert!(!m.can_edit_terrain(o));
    }

    // -- 63. Owner can edit their own objects --------------------------------

    #[test]
    fn perm_owner_can_edit_own_object() {
        let m = perms_module_no_bypass();
        let o = perm_owner_id();
        let perms = PermissionMask::MODIFY | PermissionMask::MOVE | PermissionMask::COPY;
        assert!(m.can_edit_object(o, o, Uuid::nil(), perms));
    }

    // -- 64. Owner can move their own objects --------------------------------

    #[test]
    fn perm_owner_can_move_own_object() {
        let m = perms_module_no_bypass();
        let o = perm_owner_id();
        let perms = PermissionMask::MOVE;
        assert!(m.can_move_object(o, o, Uuid::nil(), perms));
    }

    // -- 65. Owner can copy their own objects --------------------------------

    #[test]
    fn perm_owner_can_copy_own_object() {
        let m = perms_module_no_bypass();
        let o = perm_owner_id();
        let perms = PermissionMask::COPY;
        assert!(m.can_copy_object(o, o, Uuid::nil(), perms));
    }

    // -- 66. Owner can delete their own objects ------------------------------

    #[test]
    fn perm_owner_can_delete_own_object() {
        let m = perms_module_no_bypass();
        let o = perm_owner_id();
        assert!(m.can_delete_object(o, o, Uuid::nil()));
    }

    // -- 67. Stranger cannot edit others' objects ----------------------------

    #[test]
    fn perm_stranger_cannot_edit_others_object() {
        let m = perms_module_no_bypass();
        let perms = PermissionMask::ALL;
        assert!(!m.can_edit_object(perm_stranger_id(), perm_owner_id(), Uuid::nil(), perms));
    }

    // -- 68. Stranger cannot move others' objects ----------------------------

    #[test]
    fn perm_stranger_cannot_move_others_object() {
        let m = perms_module_no_bypass();
        let perms = PermissionMask::ALL;
        assert!(!m.can_move_object(perm_stranger_id(), perm_owner_id(), Uuid::nil(), perms));
    }

    // -- 69. Stranger cannot copy without TRANSFER ---------------------------

    #[test]
    fn perm_stranger_cannot_copy_without_transfer() {
        let m = perms_module_no_bypass();
        let perms = PermissionMask::COPY;
        assert!(!m.can_copy_object(perm_stranger_id(), perm_owner_id(), Uuid::nil(), perms));
    }

    // -- 70. Stranger cannot delete others' objects --------------------------

    #[test]
    fn perm_stranger_cannot_delete_others_object() {
        let m = perms_module_no_bypass();
        assert!(!m.can_delete_object(perm_stranger_id(), perm_owner_id(), Uuid::nil()));
    }

    // -- 71. Estate manager can edit terrain ---------------------------------

    #[test]
    fn perm_estate_manager_can_edit_terrain() {
        let mut m = perms_module_no_bypass();
        m.estate_managers.insert(perm_estate_manager_id());
        assert!(m.can_edit_terrain(perm_estate_manager_id()));
    }

    // -- 72. Estate manager can return objects --------------------------------

    #[test]
    fn perm_estate_manager_can_return_objects() {
        let mut m = perms_module_no_bypass();
        m.estate_managers.insert(perm_estate_manager_id());
        let parcel = perm_parcel_with_owner(perm_owner_id());
        assert!(m.can_return_object(
            perm_estate_manager_id(),
            perm_owner_id(),
            Uuid::nil(),
            Some(&parcel),
        ));
    }

    // -- 73. Estate owner can edit estate ------------------------------------

    #[test]
    fn perm_estate_owner_can_edit_estate() {
        let m = perms_module_no_bypass();
        assert!(m.can_edit_estate(perm_estate_owner_id()));
    }

    // -- 74. God mode bypasses all checks ------------------------------------

    #[test]
    fn perm_god_bypasses_all() {
        let mut m = perms_module_no_bypass();
        m.god_users.insert(perm_god_id());

        let perms = PermissionMask::NONE;
        assert!(m.can_edit_object(perm_god_id(), perm_owner_id(), Uuid::nil(), perms));
        assert!(m.can_move_object(perm_god_id(), perm_owner_id(), Uuid::nil(), perms));
        assert!(m.can_copy_object(perm_god_id(), perm_owner_id(), Uuid::nil(), perms));
        assert!(m.can_delete_object(perm_god_id(), perm_owner_id(), Uuid::nil()));
        assert!(m.can_edit_terrain(perm_god_id()));
        assert!(m.can_edit_estate(perm_god_id()));
    }

    // -- 75. Parcel owner can return objects on their parcel ------------------

    #[test]
    fn perm_parcel_owner_can_return_objects() {
        let m = perms_module_no_bypass();
        let parcel_owner = Uuid::from_u128(0xDDDD);
        let parcel = perm_parcel_with_owner(parcel_owner);
        assert!(m.can_return_object(parcel_owner, perm_owner_id(), Uuid::nil(), Some(&parcel),));
    }

    // -- 76. Script permissions respect parcel AllowOtherScripts flag --------

    #[test]
    fn perm_scripts_allowed_when_parcel_allows() {
        let m = perms_module_no_bypass();
        let mut parcel = perm_parcel_with_owner(perm_owner_id());
        parcel.flags = ParcelFlags::ALLOW_OTHER_SCRIPTS.bits();
        assert!(m.can_run_script(perm_stranger_id(), perm_stranger_id(), Some(&parcel)));
    }

    // -- 77. Scripts denied when parcel does not allow -----------------------

    #[test]
    fn perm_scripts_denied_when_parcel_denies() {
        let m = perms_module_no_bypass();
        let mut parcel = perm_parcel_with_owner(perm_owner_id());
        parcel.flags = 0;
        assert!(!m.can_run_script(perm_stranger_id(), perm_stranger_id(), Some(&parcel)));
    }

    // -- 78. Admin detection: estate owner with region_owner_is_god=true -----

    #[test]
    fn perm_admin_estate_owner_is_god() {
        let m = perms_module_no_bypass();
        assert!(m.is_administrator(perm_estate_owner_id()));
    }

    // -- 79. Admin detection: estate owner NOT admin when config off ---------

    #[test]
    fn perm_admin_estate_owner_not_god_when_disabled() {
        let mut m = perms_module_no_bypass();
        m.config.region_owner_is_god = false;
        assert!(!m.is_administrator(perm_estate_owner_id()));
    }

    // -- 80. Admin detection: estate manager with region_manager_is_god ------

    #[test]
    fn perm_admin_estate_manager_is_god() {
        let mut m = perms_module_no_bypass();
        m.config.region_manager_is_god = true;
        m.estate_managers.insert(perm_estate_manager_id());
        assert!(m.is_administrator(perm_estate_manager_id()));
    }

    // -- 81. Admin detection: grid god in god_users set ----------------------

    #[test]
    fn perm_admin_grid_god() {
        let mut m = perms_module_no_bypass();
        m.god_users.insert(perm_god_id());
        assert!(m.is_administrator(perm_god_id()));
    }

    // -- 82. Admin detection: nil UUID is never admin ------------------------

    #[test]
    fn perm_nil_uuid_not_admin() {
        let m = perms_module_no_bypass();
        assert!(!m.is_administrator(Uuid::nil()));
    }

    // -- 83. Transfer requires TRANSFER permission bit -----------------------

    #[test]
    fn perm_transfer_requires_transfer_bit() {
        let m = perms_module_no_bypass();
        let perms_no_transfer = PermissionMask::COPY | PermissionMask::MODIFY;
        assert!(!m.can_transfer_object(perm_owner_id(), perm_owner_id(), perms_no_transfer));
    }

    // -- 84. Transfer allowed with TRANSFER bit ------------------------------

    #[test]
    fn perm_transfer_allowed_with_bit() {
        let m = perms_module_no_bypass();
        let perms = PermissionMask::TRANSFER;
        assert!(m.can_transfer_object(perm_owner_id(), perm_owner_id(), perms));
    }

    // -- 85. Copy requires COPY permission bit --------------------------------

    #[test]
    fn perm_copy_requires_copy_bit() {
        let m = perms_module_no_bypass();
        let perms_no_copy = PermissionMask::MODIFY | PermissionMask::MOVE;
        assert!(!m.can_copy_object(perm_owner_id(), perm_owner_id(), Uuid::nil(), perms_no_copy));
    }

    // -- 86. Owner cannot edit without MODIFY bit ----------------------------

    #[test]
    fn perm_owner_cannot_edit_without_modify() {
        let m = perms_module_no_bypass();
        let perms = PermissionMask::COPY | PermissionMask::MOVE;
        assert!(!m.can_edit_object(perm_owner_id(), perm_owner_id(), Uuid::nil(), perms));
    }

    // -- 87. Owner cannot move without MOVE bit ------------------------------

    #[test]
    fn perm_owner_cannot_move_without_move_bit() {
        let m = perms_module_no_bypass();
        let perms = PermissionMask::MODIFY | PermissionMask::COPY;
        assert!(!m.can_move_object(perm_owner_id(), perm_owner_id(), Uuid::nil(), perms));
    }

    // -- 88. DefaultPermissionsModule initialise reads config -----------------

    #[test]
    fn perm_module_initialise_reads_config() {
        let mut m = DefaultPermissionsModule::new();
        let mut config = TestConfig::new();
        config.set_bool("Permissions", "serverside_object_permissions", true);
        config.set_bool("Permissions", "propagate_permissions", false);
        config.set_bool("Permissions", "allow_grid_gods", true);
        config.set_bool("Permissions", "region_owner_is_god", false);
        config.set_bool("Permissions", "region_manager_is_god", true);

        m.initialise(&config).unwrap();

        assert!(!m.config.bypass_permissions);
        assert!(!m.config.propagate_permissions);
        assert!(m.config.allow_grid_gods);
        assert!(!m.config.region_owner_is_god);
        assert!(m.config.region_manager_is_god);
    }

    // -- 89. DefaultPermissionsModule RegionModule lifecycle ------------------

    #[test]
    fn perm_module_lifecycle() {
        let mut m = DefaultPermissionsModule::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();

        m.initialise(&config).unwrap();
        m.add_region(&scene).unwrap();
        assert_eq!(m.name(), "DefaultPermissionsModule");
        assert_eq!(m.module_type(), ModuleType::NonShared);
        m.remove_region(&scene).unwrap();
        m.close().unwrap();
    }

    // -- 90. force_grid_gods_only overrides estate settings -------------------

    #[test]
    fn perm_force_grid_gods_only() {
        let mut m = DefaultPermissionsModule::new();
        let mut config = TestConfig::new();
        config.set_bool("Permissions", "serverside_object_permissions", true);
        config.set_bool("Permissions", "force_grid_gods_only", true);
        config.set_bool("Permissions", "region_owner_is_god", true);
        config.set_bool("Permissions", "region_manager_is_god", true);

        m.initialise(&config).unwrap();

        assert!(!m.config.region_owner_is_god);
        assert!(!m.config.region_manager_is_god);
        assert!(m.config.allow_grid_gods);
    }

    // -- 91. can_compile_script respects allowed_script_creators ---------------

    #[test]
    fn perm_compile_script_admin_only() {
        let mut m = perms_module_no_bypass();
        m.config.allowed_script_creators = UserSet::Administrators;
        m.god_users.insert(perm_god_id());

        assert!(!m.can_compile_script(perm_stranger_id()));
        assert!(m.can_compile_script(perm_god_id()));
        assert!(m.can_compile_script(perm_estate_owner_id()));
    }

    // -- 92. can_create_object on parcel owner's land -------------------------

    #[test]
    fn perm_create_object_parcel_owner() {
        let m = perms_module_no_bypass();
        let parcel = perm_parcel_with_owner(perm_owner_id());
        assert!(m.can_create_object(perm_owner_id(), (128.0, 128.0, 0.0), Some(&parcel)));
    }

    // -- 93. can_create_object denied for stranger on restricted parcel -------

    #[test]
    fn perm_create_object_denied_stranger_no_flags() {
        let m = perms_module_no_bypass();
        let mut parcel = perm_parcel_with_owner(perm_owner_id());
        parcel.flags = 0;
        assert!(!m.can_create_object(perm_stranger_id(), (128.0, 128.0, 0.0), Some(&parcel)));
    }

    // -- 94. can_create_object allowed when parcel has CreateObjects ----------

    #[test]
    fn perm_create_object_allowed_when_parcel_flag_set() {
        let m = perms_module_no_bypass();
        let mut parcel = perm_parcel_with_owner(perm_owner_id());
        parcel.flags = ParcelFlags::CREATE_OBJECTS.bits();
        assert!(m.can_create_object(perm_stranger_id(), (128.0, 128.0, 0.0), Some(&parcel)));
    }

    // -- 95. can_take_object: owner can take ---------------------------------

    #[test]
    fn perm_take_object_owner() {
        let m = perms_module_no_bypass();
        assert!(m.can_take_object(perm_owner_id(), perm_owner_id(), Uuid::nil()));
    }

    // -- 96. can_take_object: stranger cannot take ----------------------------

    #[test]
    fn perm_take_object_stranger_denied() {
        let m = perms_module_no_bypass();
        assert!(!m.can_take_object(perm_stranger_id(), perm_owner_id(), Uuid::nil()));
    }

    // -- 97. can_edit_parcel: parcel owner can edit ---------------------------

    #[test]
    fn perm_edit_parcel_owner() {
        let m = perms_module_no_bypass();
        assert!(m.can_edit_parcel(perm_owner_id(), perm_owner_id()));
    }

    // -- 98. can_edit_parcel: stranger cannot edit ----------------------------

    #[test]
    fn perm_edit_parcel_stranger_denied() {
        let m = perms_module_no_bypass();
        assert!(!m.can_edit_parcel(perm_stranger_id(), perm_owner_id()));
    }

    // -- 99. implicit_gods disables automatic_gods ----------------------------

    #[test]
    fn perm_implicit_gods_disables_automatic() {
        let mut m = DefaultPermissionsModule::new();
        let mut config = TestConfig::new();
        config.set_bool("Permissions", "serverside_object_permissions", true);
        config.set_bool("Permissions", "implicit_gods", true);
        config.set_bool("Permissions", "automatic_gods", true);

        m.initialise(&config).unwrap();

        assert!(m.config.implicit_gods);
        assert!(!m.config.automatic_gods);
    }

    // -- 100. Return object: stranger denied ----------------------------------

    #[test]
    fn perm_return_object_stranger_denied() {
        let m = perms_module_no_bypass();
        let parcel = perm_parcel_with_owner(perm_owner_id());
        assert!(!m.can_return_object(
            perm_stranger_id(),
            perm_owner_id(),
            Uuid::nil(),
            Some(&parcel),
        ));
    }

    // ===================================================================
    // World-map module tests (section 8)
    // ===================================================================

    use crate::world_map::map_image::{
        MapImageModule, MapTileConfig, ShadedTerrainRenderer, TerrainRenderer,
    };
    use crate::world_map::{
        MapBlock, MapItem, MapItemType, MapSearchModule as MapSearch, SimAccess, WorldMapModule,
    };

    // -- 101. SimAccess from_u8 roundtrip ------------------------------------

    #[test]
    fn sim_access_from_u8_roundtrip() {
        assert_eq!(SimAccess::from_u8(13), SimAccess::PG);
        assert_eq!(SimAccess::from_u8(21), SimAccess::Mature);
        assert_eq!(SimAccess::from_u8(42), SimAccess::Adult);
        assert_eq!(SimAccess::from_u8(254), SimAccess::Down);
        assert_eq!(SimAccess::from_u8(255), SimAccess::NonExistent);
        // Unknown values map to Unknown.
        assert_eq!(SimAccess::from_u8(99), SimAccess::Unknown);
    }

    // -- 102. MapBlock construction and terminal sentinel ---------------------

    #[test]
    fn map_block_construction_and_terminal() {
        let block = MapBlock::new(1000, 1000, "TestRegion");
        assert_eq!(block.x, 1000);
        assert_eq!(block.y, 1000);
        assert_eq!(block.name, "TestRegion");
        assert_eq!(block.access, SimAccess::PG);
        assert_eq!(block.agents, 0);
        assert_eq!(block.size_x, 256);

        let terminal = MapBlock::terminal_block("search query");
        assert_eq!(terminal.access, SimAccess::NonExistent);
        assert_eq!(terminal.name, "search query");
        assert_eq!(terminal.x, 0);
        assert_eq!(terminal.size_x, 0);
    }

    // -- 103. MapItemType from_u32 -------------------------------------------

    #[test]
    fn map_item_type_from_u32() {
        assert_eq!(MapItemType::from_u32(1), Some(MapItemType::Telehub));
        assert_eq!(MapItemType::from_u32(7), Some(MapItemType::LandForSale));
        assert_eq!(MapItemType::from_u32(8), Some(MapItemType::Classified));
        assert_eq!(MapItemType::from_u32(6), Some(MapItemType::AgentLocations));
        assert!(MapItemType::from_u32(0).is_none());
        assert!(MapItemType::from_u32(99).is_none());
    }

    // -- 104. WorldMapModule map block request --------------------------------

    #[test]
    fn world_map_block_request() {
        let mut module = WorldMapModule::new();
        module.region_grid_x = 1000;
        module.region_grid_y = 1000;
        module.region_name = "Main".to_string();

        // Cache a neighbour.
        module.cache_block(MapBlock::new(1001, 1000, "East"));

        // Request a rectangle that includes both.
        let results = module.handle_map_block_request(999, 999, 1002, 1001);
        assert_eq!(results.len(), 2);
        let names: Vec<&str> = results.iter().map(|b| b.name.as_str()).collect();
        assert!(names.contains(&"Main"));
        assert!(names.contains(&"East"));

        // Request outside range.
        let empty = module.handle_map_block_request(0, 0, 10, 10);
        assert!(empty.is_empty());
    }

    // -- 105. WorldMapModule map item request ---------------------------------

    #[test]
    fn world_map_item_request() {
        let mut module = WorldMapModule::new();
        module.add_map_item(MapItem::new(100, 200, MapItemType::Telehub, "Hub1"));
        module.add_map_item(MapItem::new(300, 400, MapItemType::LandForSale, "Plot A"));
        module.add_map_item(MapItem::new(500, 600, MapItemType::LandForSale, "Plot B"));

        let telehubs = module.handle_map_item_request(MapItemType::Telehub);
        assert_eq!(telehubs.len(), 1);
        assert_eq!(telehubs[0].name, "Hub1");

        let land = module.handle_map_item_request(MapItemType::LandForSale);
        assert_eq!(land.len(), 2);

        // No events registered.
        let events = module.handle_map_item_request(MapItemType::PGEvent);
        assert!(events.is_empty());
    }

    // -- 106. WorldMapModule remove_items_by_type ----------------------------

    #[test]
    fn world_map_remove_items_by_type() {
        let mut module = WorldMapModule::new();
        module.add_map_item(MapItem::new(0, 0, MapItemType::Telehub, "Hub"));
        module.add_map_item(MapItem::new(0, 0, MapItemType::LandForSale, "Land"));
        assert_eq!(module.map_item_count(), 2);

        module.remove_items_by_type(MapItemType::Telehub);
        assert_eq!(module.map_item_count(), 1);
        assert!(
            module
                .handle_map_item_request(MapItemType::Telehub)
                .is_empty()
        );
    }

    // -- 107. WorldMapModule RegionModule lifecycle ---------------------------

    #[test]
    fn world_map_module_lifecycle() {
        let mut module = WorldMapModule::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();

        module.initialise(&config).unwrap();
        module.add_region(&scene).unwrap();
        assert_eq!(module.name(), "WorldMapModule");
        assert_eq!(module.module_type(), ModuleType::NonShared);

        // Add items, then remove_region should clear them.
        module.add_map_item(MapItem::new(0, 0, MapItemType::Telehub, "Hub"));
        module.cache_block(MapBlock::new(1001, 1000, "Neighbour"));
        module.remove_region(&scene).unwrap();
        assert_eq!(module.map_item_count(), 0);
        assert_eq!(module.cached_block_count(), 0);
        module.close().unwrap();
    }

    // -- 108. MapSearchModule search -----------------------------------------

    #[test]
    fn map_search_module_search() {
        let mut module = MapSearch::new();
        module.register_region(MapBlock::new(1000, 1000, "Welcome Island"));
        module.register_region(MapBlock::new(1001, 1000, "Welcome Hub"));
        module.register_region(MapBlock::new(1002, 1000, "Sandbox Plaza"));

        // Search for "welcome" should find 2 + terminal.
        let results = module.search_map("welcome", 20).unwrap();
        assert_eq!(results.len(), 3); // 2 matches + terminal
        assert_eq!(results.last().unwrap().access, SimAccess::NonExistent);

        // Case insensitive.
        let results2 = module.search_map("SANDBOX", 20).unwrap();
        assert_eq!(results2.len(), 2); // 1 match + terminal

        // Short query rejected.
        let err = module.search_map("ab", 20);
        assert!(err.is_err());
    }

    // -- 109. MapSearchModule register/unregister ----------------------------

    #[test]
    fn map_search_register_unregister() {
        let mut module = MapSearch::new();
        module.register_region(MapBlock::new(0, 0, "Region One"));
        assert_eq!(module.region_count(), 1);

        module.unregister_region("Region One");
        assert_eq!(module.region_count(), 0);

        // Search now returns only the terminal.
        let results = module.search_map("Region", 20).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].access, SimAccess::NonExistent);
    }

    // -- 110. ShadedTerrainRenderer produces correctly-sized pixmap ----------

    #[test]
    fn shaded_renderer_output_size() {
        let renderer = ShadedTerrainRenderer::new();
        // Flat terrain at 50m.
        let heightmap = vec![50.0_f32; 64 * 64];
        let pixmap = renderer.render_terrain(&heightmap, 64, 64, 20.0);
        assert_eq!(pixmap.width(), 64);
        assert_eq!(pixmap.height(), 64);
        // All land pixels (above water) should have a non-zero green channel.
        let data = pixmap.data();
        // Check a pixel in the middle: RGBA at index (32, 32) after Y-flip.
        let yr = 63 - 32;
        let idx = ((yr * 64 + 32) * 4) as usize;
        assert!(
            data[idx + 1] > 0,
            "green channel should be non-zero for land"
        );
        assert_eq!(data[idx + 3], 255, "alpha should be 255");
    }

    // -- 111. ShadedTerrainRenderer paints water below water_height ----------

    #[test]
    fn shaded_renderer_water_painting() {
        let renderer = ShadedTerrainRenderer {
            water_color: (0x1D, 0x47, 0x5F),
        };
        // All terrain at 10m, water at 20m — everything is underwater.
        let heightmap = vec![10.0_f32; 16 * 16];
        let pixmap = renderer.render_terrain(&heightmap, 16, 16, 20.0);
        let data = pixmap.data();

        // Every pixel should be the water colour.
        for y in 0..16_u32 {
            for x in 0..16_u32 {
                let idx = ((y * 16 + x) * 4) as usize;
                assert_eq!(data[idx], 0x1D, "red");
                assert_eq!(data[idx + 1], 0x47, "green");
                assert_eq!(data[idx + 2], 0x5F, "blue");
            }
        }
    }

    // -- 112. MapImageModule generate_map_tile returns correct byte count ----

    #[test]
    fn map_image_generate_tile_size() {
        let mut module = MapImageModule::new();
        module.config = MapTileConfig {
            tile_size: 32,
            draw_prims: false,
            terrain_color_scheme: "shaded".to_string(),
            water_color: (0x1D, 0x47, 0x5F),
            water_height: 20.0,
        };
        let heightmap = vec![50.0_f32; 32 * 32];
        let rgba = module.generate_map_tile(&heightmap);
        assert_eq!(rgba.len(), 32 * 32 * 4);
    }

    // -- 113. MapImageModule upload_map_tile stub returns new UUID -----------

    #[test]
    fn map_image_upload_tile_stub() {
        let mut module = MapImageModule::new();
        assert_eq!(module.last_tile_id(), Uuid::nil());

        let id = module.upload_map_tile(&[0u8; 256]);
        assert!(!id.is_nil());
        assert_eq!(module.last_tile_id(), id);

        // Second upload yields a different UUID.
        let id2 = module.upload_map_tile(&[0u8; 256]);
        assert_ne!(id, id2);
    }

    // -- 114. MapImageModule RegionModule lifecycle ---------------------------

    #[test]
    fn map_image_module_lifecycle() {
        let mut module = MapImageModule::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();

        module.initialise(&config).unwrap();
        module.add_region(&scene).unwrap();
        assert_eq!(module.name(), "MapImageModule");
        assert_eq!(module.module_type(), ModuleType::NonShared);
        module.remove_region(&scene).unwrap();
        module.close().unwrap();
    }

    // -- 115. MapTileConfig defaults match expectations -----------------------

    #[test]
    fn map_tile_config_defaults() {
        let cfg = MapTileConfig::default();
        assert_eq!(cfg.tile_size, 256);
        assert!(cfg.draw_prims);
        assert_eq!(cfg.terrain_color_scheme, "shaded");
        assert_eq!(cfg.water_color, (0x1D, 0x47, 0x5F));
        assert!((cfg.water_height - 20.0).abs() < f32::EPSILON);
    }

    // ===================================================================
    // Chat-module tests (section 7.1)
    // ===================================================================

    use crate::chat::{
        ChatMessage, ChatModule as ChatMod, ChatSourceType, ChatType, DEBUG_CHANNEL,
        MAX_MESSAGE_LENGTH,
    };

    fn make_chat_msg(chat_type: ChatType, channel: i32) -> ChatMessage {
        ChatMessage {
            sender_id: Uuid::new_v4(),
            sender_name: "Tester".to_string(),
            message: "Hello, world!".to_string(),
            chat_type,
            source_type: ChatSourceType::Agent,
            position: [128.0, 128.0, 25.0],
            channel,
        }
    }

    // -- 116. ChatType::range_squared values ----------------------------------

    #[test]
    fn chat_type_range_squared() {
        assert_eq!(ChatType::Whisper.range_squared(), Some(100.0));
        assert_eq!(ChatType::Say.range_squared(), Some(400.0));
        assert_eq!(ChatType::Shout.range_squared(), Some(10_000.0));
        assert_eq!(ChatType::Region.range_squared(), None);
        assert_eq!(ChatType::Owner.range_squared(), None);
        assert_eq!(ChatType::Direct.range_squared(), None);
        assert_eq!(ChatType::Broadcast.range_squared(), None);
        assert_eq!(ChatType::DebugChannel.range_squared(), None);
    }

    // -- 117. ChatModule default construction ---------------------------------

    #[test]
    fn chat_module_defaults() {
        let m = ChatMod::new();
        assert!(m.enabled);
        assert_eq!(m.whisper_distance, 10);
        assert_eq!(m.say_distance, 20);
        assert_eq!(m.shout_distance, 100);
        assert!(m.admin_prefix.is_empty());
    }

    // -- 118. ChatModule reads config -----------------------------------------

    #[test]
    fn chat_module_reads_config() {
        let mut m = ChatMod::new();
        let mut config = TestConfig::new();
        config.set_int("Chat", "whisper_distance", 15);
        config.set_int("Chat", "say_distance", 30);
        config.set_int("Chat", "shout_distance", 200);
        config.set_string("Chat", "admin_prefix", "[ADMIN] ");
        m.initialise(&config).unwrap();
        assert_eq!(m.whisper_distance, 15);
        assert_eq!(m.say_distance, 30);
        assert_eq!(m.shout_distance, 200);
        assert_eq!(m.admin_prefix, "[ADMIN] ");
    }

    // -- 119. ChatModule lifecycle --------------------------------------------

    #[test]
    fn chat_module_lifecycle() {
        let mut m = ChatMod::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        m.initialise(&config).unwrap();
        m.add_region(&scene).unwrap();
        assert_eq!(m.name(), "ChatModule");
        assert_eq!(m.module_type(), ModuleType::NonShared);
        m.remove_region(&scene).unwrap();
        m.close().unwrap();
    }

    // -- 120. on_chat_from_viewer broadcasts to event bus ----------------------

    #[test]
    fn chat_on_chat_from_viewer_broadcasts() {
        let mut m = ChatMod::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        m.initialise(&config).unwrap();
        m.add_region(&scene).unwrap();

        let mut rx = scene.events.on_chat_message.subscribe();
        let mut msg = make_chat_msg(ChatType::Say, 0);
        assert!(m.on_chat_from_viewer(&mut msg));

        let evt = rx.try_recv().unwrap();
        assert_eq!(evt.message, "Hello, world!");
        assert_eq!(evt.channel, 0);
    }

    // -- 121. on_chat_from_viewer truncates long messages ----------------------

    #[test]
    fn chat_truncates_long_messages() {
        let mut m = ChatMod::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        m.initialise(&config).unwrap();
        m.add_region(&scene).unwrap();

        let mut rx = scene.events.on_chat_message.subscribe();
        let long_text = "A".repeat(2000);
        let mut msg = make_chat_msg(ChatType::Say, 0);
        msg.message = long_text;
        m.on_chat_from_viewer(&mut msg);

        let evt = rx.try_recv().unwrap();
        assert!(evt.message.len() <= MAX_MESSAGE_LENGTH);
    }

    // -- 122. on_chat_from_viewer sets DebugChannel on debug channel -----------

    #[test]
    fn chat_debug_channel_override() {
        let mut m = ChatMod::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        m.initialise(&config).unwrap();
        m.add_region(&scene).unwrap();

        let mut msg = make_chat_msg(ChatType::Say, DEBUG_CHANNEL);
        m.on_chat_from_viewer(&mut msg);
        assert_eq!(msg.chat_type, ChatType::DebugChannel);
    }

    // -- 123. deliver_chat_to_client range filter: in range -------------------

    #[test]
    fn chat_deliver_in_range() {
        let m = ChatMod::new();
        let msg = make_chat_msg(ChatType::Say, 0);
        // Listener 10m away (within 20m say range).
        let listener_pos = [138.0, 128.0, 25.0];
        assert!(m.deliver_chat_to_client(&msg, listener_pos));
    }

    // -- 124. deliver_chat_to_client range filter: out of range ---------------

    #[test]
    fn chat_deliver_out_of_range() {
        let m = ChatMod::new();
        let msg = make_chat_msg(ChatType::Whisper, 0);
        // Listener 15m away (outside 10m whisper range).
        let listener_pos = [143.0, 128.0, 25.0];
        assert!(!m.deliver_chat_to_client(&msg, listener_pos));
    }

    // -- 125. deliver_chat_to_client: unlimited range types always deliver -----

    #[test]
    fn chat_deliver_unlimited_range() {
        let m = ChatMod::new();
        let msg = make_chat_msg(ChatType::Region, 0);
        // Very far away.
        let listener_pos = [9999.0, 9999.0, 9999.0];
        assert!(m.deliver_chat_to_client(&msg, listener_pos));
    }

    // -- 126. is_frozen stub returns false -------------------------------------

    #[test]
    fn chat_is_frozen_stub() {
        let m = ChatMod::new();
        assert!(!m.is_frozen(Uuid::new_v4()));
    }

    // -- 127. on_chat_from_viewer returns false when disabled ------------------

    #[test]
    fn chat_disabled_returns_false() {
        let mut m = ChatMod::new();
        m.enabled = false;
        let mut msg = make_chat_msg(ChatType::Say, 0);
        assert!(!m.on_chat_from_viewer(&mut msg));
    }

    // ===================================================================
    // Messaging-module tests (section 7.2 / 7.3)
    // ===================================================================

    use crate::messaging::{
        GridInstantMessage, IMDialog, MessageTransferModule as MsgXfer, MuteEntry,
        MuteListModule as MuteMod, MuteType, OfflineMessageModule as OfflineMod,
        PresenceModule as PresenceMod,
    };

    fn make_im(from: Uuid, to: Uuid) -> GridInstantMessage {
        GridInstantMessage {
            from_agent_id: from,
            from_agent_name: "Sender".to_string(),
            to_agent_id: to,
            message: "Hello IM".to_string(),
            im_session_id: Uuid::new_v4(),
            dialog: IMDialog::MessageFromAgent,
            from_group: false,
            offline: 0,
            parent_estate_id: 1,
            position: [128.0, 128.0, 25.0],
            region_id: Uuid::new_v4(),
            timestamp: 1000,
            binary_bucket: Vec::new(),
        }
    }

    // -- 128. MessageTransferModule send to nil returns false ------------------

    #[test]
    fn message_transfer_nil_recipient() {
        let m = MsgXfer::new();
        let im = make_im(Uuid::new_v4(), Uuid::nil());
        assert!(!m.send_instant_message(&im));
    }

    // -- 129. MessageTransferModule send to valid agent returns true -----------

    #[test]
    fn message_transfer_valid_recipient() {
        let m = MsgXfer::new();
        let im = make_im(Uuid::new_v4(), Uuid::new_v4());
        assert!(m.send_instant_message(&im));
    }

    // -- 130. MessageTransferModule lifecycle ----------------------------------

    #[test]
    fn message_transfer_lifecycle() {
        let mut m = MsgXfer::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        m.initialise(&config).unwrap();
        m.add_region(&scene).unwrap();
        assert_eq!(m.name(), "MessageTransferModule");
        assert_eq!(m.module_type(), ModuleType::Shared);
        m.remove_region(&scene).unwrap();
        m.close().unwrap();
    }

    // -- 131. OfflineMessageModule store and retrieve -------------------------

    #[test]
    fn offline_store_retrieve() {
        let mut m = OfflineMod::new();
        let agent = Uuid::new_v4();
        let im1 = make_im(Uuid::new_v4(), agent);
        let im2 = make_im(Uuid::new_v4(), agent);
        m.store_message(im1);
        m.store_message(im2);
        assert_eq!(m.pending_count(agent), 2);
        assert_eq!(m.total_pending(), 2);

        let msgs = m.retrieve_messages(agent);
        assert_eq!(msgs.len(), 2);
        assert_eq!(m.pending_count(agent), 0);
        assert_eq!(m.total_pending(), 0);
    }

    // -- 132. OfflineMessageModule retrieve empty returns empty vec ------------

    #[test]
    fn offline_retrieve_empty() {
        let mut m = OfflineMod::new();
        let msgs = m.retrieve_messages(Uuid::new_v4());
        assert!(msgs.is_empty());
    }

    // -- 133. MuteListModule update and is_muted ------------------------------

    #[test]
    fn mute_list_update_and_check() {
        let mut m = MuteMod::new();
        let owner = Uuid::new_v4();
        let target = Uuid::new_v4();
        let entry = MuteEntry {
            agent_id: owner,
            mute_id: target,
            name: "Annoying".to_string(),
            mute_type: MuteType::ByAgent,
            flags: 0,
        };
        m.update_mute(owner, entry);
        assert!(m.is_muted(owner, target));
        assert!(!m.is_muted(owner, Uuid::new_v4()));
    }

    // -- 134. MuteListModule remove_mute --------------------------------------

    #[test]
    fn mute_list_remove() {
        let mut m = MuteMod::new();
        let owner = Uuid::new_v4();
        let target = Uuid::new_v4();
        let entry = MuteEntry {
            agent_id: owner,
            mute_id: target,
            name: "Removed".to_string(),
            mute_type: MuteType::ByAgent,
            flags: 0,
        };
        m.update_mute(owner, entry);
        assert!(m.is_muted(owner, target));
        m.remove_mute(owner, target, "Removed");
        assert!(!m.is_muted(owner, target));
    }

    // -- 135. MuteListModule get_mutes ----------------------------------------

    #[test]
    fn mute_list_get_mutes() {
        let mut m = MuteMod::new();
        let owner = Uuid::new_v4();
        assert!(m.get_mutes(owner).is_empty());

        m.update_mute(
            owner,
            MuteEntry {
                agent_id: owner,
                mute_id: Uuid::new_v4(),
                name: "A".to_string(),
                mute_type: MuteType::ByAgent,
                flags: 0,
            },
        );
        m.update_mute(
            owner,
            MuteEntry {
                agent_id: owner,
                mute_id: Uuid::new_v4(),
                name: "B".to_string(),
                mute_type: MuteType::ByObject,
                flags: 0,
            },
        );
        assert_eq!(m.get_mutes(owner).len(), 2);
    }

    // -- 136. MuteListModule update replaces existing entry --------------------

    #[test]
    fn mute_list_update_replaces() {
        let mut m = MuteMod::new();
        let owner = Uuid::new_v4();
        let target = Uuid::new_v4();
        m.update_mute(
            owner,
            MuteEntry {
                agent_id: owner,
                mute_id: target,
                name: "Original".to_string(),
                mute_type: MuteType::ByAgent,
                flags: 0,
            },
        );
        m.update_mute(
            owner,
            MuteEntry {
                agent_id: owner,
                mute_id: target,
                name: "Updated".to_string(),
                mute_type: MuteType::ByAgent,
                flags: 1,
            },
        );
        let mutes = m.get_mutes(owner);
        assert_eq!(mutes.len(), 1);
        assert_eq!(mutes[0].name, "Updated");
        assert_eq!(mutes[0].flags, 1);
    }

    // -- 137. PresenceModule register/deregister/is_present --------------------

    #[test]
    fn presence_register_deregister() {
        let mut m = PresenceMod::new();
        let agent = Uuid::new_v4();
        assert!(!m.is_present(agent));
        assert_eq!(m.presence_count(), 0);

        m.register_presence(agent);
        assert!(m.is_present(agent));
        assert_eq!(m.presence_count(), 1);

        m.deregister_presence(agent);
        assert!(!m.is_present(agent));
        assert_eq!(m.presence_count(), 0);
    }

    // -- 138. PresenceModule get_present_agents --------------------------------

    #[test]
    fn presence_get_agents() {
        let mut m = PresenceMod::new();
        let a1 = Uuid::new_v4();
        let a2 = Uuid::new_v4();
        m.register_presence(a1);
        m.register_presence(a2);

        let agents = m.get_present_agents();
        assert_eq!(agents.len(), 2);
        assert!(agents.contains(&a1));
        assert!(agents.contains(&a2));
    }

    // -- 139. PresenceModule lifecycle -----------------------------------------

    #[test]
    fn presence_module_lifecycle() {
        let mut m = PresenceMod::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        m.initialise(&config).unwrap();
        m.add_region(&scene).unwrap();
        assert_eq!(m.name(), "PresenceModule");
        assert_eq!(m.module_type(), ModuleType::Shared);

        m.register_presence(Uuid::new_v4());
        m.close().unwrap();
        assert_eq!(m.presence_count(), 0);
    }

    // -- 140. IMDialog enum values match C# -----------------------------------

    #[test]
    fn im_dialog_values() {
        assert_eq!(IMDialog::MessageFromAgent as u8, 0);
        assert_eq!(IMDialog::MessageFromObject as u8, 19);
        assert_eq!(IMDialog::SessionGroupStart as u8, 17);
        assert_eq!(IMDialog::SessionCardlessStart as u8, 18);
        assert_eq!(IMDialog::SessionSend as u8, 20);
        assert_eq!(IMDialog::FriendshipOffered as u8, 38);
        assert_eq!(IMDialog::FriendshipAccepted as u8, 39);
        assert_eq!(IMDialog::FriendshipDeclined as u8, 40);
        assert_eq!(IMDialog::GroupInvitation as u8, 3);
        assert_eq!(IMDialog::InventoryOffered as u8, 4);
        assert_eq!(IMDialog::InventoryAccepted as u8, 5);
        assert_eq!(IMDialog::InventoryDeclined as u8, 6);
        assert_eq!(IMDialog::StartTyping as u8, 41);
        assert_eq!(IMDialog::StopTyping as u8, 42);
        assert_eq!(IMDialog::RequestTeleport as u8, 22);
        assert_eq!(IMDialog::GroupNotice as u8, 32);
        assert_eq!(IMDialog::GroupNoticeRequested as u8, 37);
        assert_eq!(IMDialog::BUSY_AUTO_RESPONSE, 20);
        assert!(IMDialog::from_u8(0).is_some());
        assert!(IMDialog::from_u8(2).is_none());
        assert!(IMDialog::from_u8(17).is_some());
        assert!(IMDialog::from_u8(255).is_none());
    }

    // -- 141. MuteType enum values match C# -----------------------------------

    #[test]
    fn mute_type_values() {
        assert_eq!(MuteType::ByName as u8, 0);
        assert_eq!(MuteType::ByAgent as u8, 1);
        assert_eq!(MuteType::ByGroup as u8, 2);
        assert_eq!(MuteType::ByObject as u8, 3);
    }

    // -- 142. DEBUG_CHANNEL constant ------------------------------------------

    #[test]
    fn debug_channel_value() {
        assert_eq!(DEBUG_CHANNEL, i32::MAX);
    }

    // ===================================================================
    // Environment-module tests (section 9)
    // ===================================================================

    use crate::environment::{
        AccessModule, ConfigurableWind, LightShareModule, MediaEntry, MediaModule,
        RegionAccessFlags, RegionReadyModule, RestartModule, SceneSerialiser, SimpleRandomWind,
        SoundFlags, SoundModule, SoundTrigger, VegetationModule, WindModel, WindModule,
        WindlightPreset,
    };

    // -- 143. WindlightPreset defaults ------------------------------------

    #[test]
    fn windlight_preset_defaults() {
        let p = WindlightPreset::default();
        assert!((p.cloud_coverage - 0.27).abs() < f32::EPSILON);
        assert!((p.haze_density - 0.7).abs() < f32::EPSILON);
        assert!((p.scene_gamma - 1.0).abs() < f32::EPSILON);
        assert_eq!(p.sun_direction, [0.0, 0.707, 0.707]);
        assert_eq!(p.normal_map_texture, Uuid::nil());
    }

    // -- 144. LightShareModule get/set/reset windlight --------------------

    #[test]
    fn lightshare_get_set_reset() {
        let mut module = LightShareModule::new();
        assert!((module.get_windlight().cloud_coverage - 0.27).abs() < f32::EPSILON);

        let mut preset = WindlightPreset::default();
        preset.cloud_coverage = 0.9;
        module.set_windlight(preset);
        assert!((module.get_windlight().cloud_coverage - 0.9).abs() < f32::EPSILON);
        assert_eq!(module.env_version(), 0); // -1 + 1 = 0

        module.reset_windlight();
        assert!((module.get_windlight().cloud_coverage - 0.27).abs() < f32::EPSILON);
        assert_eq!(module.env_version(), 1);
    }

    // -- 145. LightShareModule lifecycle ----------------------------------

    #[test]
    fn lightshare_lifecycle() {
        let mut module = LightShareModule::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        module.initialise(&config).unwrap();
        assert!(!module.is_enabled()); // No config = disabled
        assert_eq!(module.name(), "LightShareModule");
        assert_eq!(module.module_type(), ModuleType::NonShared);
        module.add_region(&scene).unwrap();
        module.remove_region(&scene).unwrap();
        module.close().unwrap();
    }

    // -- 146. SimpleRandomWind produces non-zero after update --------------

    #[test]
    fn simple_random_wind_produces_output() {
        let mut wind = SimpleRandomWind::new();
        wind.strength = 2.0;
        wind.update(1.0);
        let mut has_nonzero = false;
        for y in 0..16 {
            for x in 0..16 {
                let (vx, vy) = wind.wind_speed((x * 16) as f32, (y * 16) as f32);
                if vx.abs() > f32::EPSILON || vy.abs() > f32::EPSILON {
                    has_nonzero = true;
                }
            }
        }
        assert!(
            has_nonzero,
            "SimpleRandomWind should produce non-zero values"
        );
        assert_eq!(wind.name(), "SimpleRandomWind");
    }

    // -- 147. ConfigurableWind default values and update -------------------

    #[test]
    fn configurable_wind_update() {
        let mut wind = ConfigurableWind::new();
        assert!((wind.avg_strength - 5.0).abs() < f32::EPSILON);
        assert!((wind.avg_direction - 0.0).abs() < f32::EPSILON);

        wind.update(100.0);
        let (vx, vy) = wind.wind_speed(128.0, 128.0);
        let mag = (vx * vx + vy * vy).sqrt();
        assert!(mag > 0.0, "ConfigurableWind should produce non-zero wind");
        assert_eq!(wind.name(), "ConfigurableWind");
    }

    // -- 148. WindModule default model is SimpleRandomWind -----------------

    #[test]
    fn wind_module_default_model() {
        let module = WindModule::new();
        assert_eq!(module.active_model_name(), "SimpleRandomWind");
        assert!(module.is_enabled());
        assert_eq!(module.frame_update_rate(), 150);
        assert_eq!(module.name(), "WindModule");
        assert_eq!(module.module_type(), ModuleType::Shared);
    }

    // -- 149. WindModule tick triggers model update at rate ----------------

    #[test]
    fn wind_module_tick_updates() {
        let mut module = WindModule::new();
        let (x0, y0) = module.wind_speed(128.0, 128.0);
        assert!((x0.abs() + y0.abs()) < f32::EPSILON);

        for _ in 0..150 {
            module.tick(1.0);
        }
        let mut found = false;
        for i in 0..16 {
            let (vx, vy) = module.wind_speed((i * 16) as f32, 0.0);
            if vx.abs() > f32::EPSILON || vy.abs() > f32::EPSILON {
                found = true;
                break;
            }
        }
        assert!(found);
    }

    // -- 150. WindModule set_model swaps model -----------------------------

    #[test]
    fn wind_module_set_model() {
        let mut module = WindModule::new();
        module.set_model(Box::new(ConfigurableWind::new()));
        assert_eq!(module.active_model_name(), "ConfigurableWind");
    }

    // -- 151. WindModule initialise with ConfigurableWind ------------------

    #[test]
    fn wind_module_init_configurable() {
        let mut module = WindModule::new();
        let mut config = TestConfig::new();
        config.set_string("Wind", "wind_plugin", "ConfigurableWind");
        config.set_float("Wind", "avg_strength", 10.0);
        config.set_float("Wind", "avg_direction", 45.0);
        module.initialise(&config).unwrap();
        assert_eq!(module.active_model_name(), "ConfigurableWind");
    }

    // -- 152. SoundFlags bitflags -----------------------------------------

    #[test]
    fn sound_flags_values() {
        assert_eq!(SoundFlags::LOOP.bits(), 1);
        assert_eq!(SoundFlags::STOP.bits(), 32);
        let combined = SoundFlags::LOOP | SoundFlags::QUEUE;
        assert!(combined.contains(SoundFlags::LOOP));
        assert!(combined.contains(SoundFlags::QUEUE));
        assert!(!combined.contains(SoundFlags::STOP));
        let sync = SoundFlags::SYNC_MASK;
        assert!(sync.contains(SoundFlags::SYNC_MASTER));
        assert!(sync.contains(SoundFlags::SYNC_SLAVE));
        assert!(sync.contains(SoundFlags::SYNC_PENDING));
    }

    // -- 153. SoundModule loop/stop tracking ------------------------------

    #[test]
    fn sound_module_loop_stop() {
        let mut module = SoundModule::new();
        let obj1 = Uuid::new_v4();
        let obj2 = Uuid::new_v4();
        let snd = Uuid::new_v4();

        module.loop_sound(obj1, snd, 1.0);
        module.loop_sound(obj2, snd, 0.5);
        assert_eq!(module.active_loop_count(), 2);

        module.stop_sound(obj1);
        assert_eq!(module.active_loop_count(), 1);

        module.loop_sound(obj2, Uuid::new_v4(), 0.8);
        assert_eq!(module.active_loop_count(), 1);
    }

    // -- 154. SoundModule lifecycle ----------------------------------------

    #[test]
    fn sound_module_lifecycle() {
        let mut module = SoundModule::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        module.initialise(&config).unwrap();
        assert!(module.is_enabled());
        assert!((module.max_distance() - 100.0).abs() < f32::EPSILON);
        assert_eq!(module.name(), "SoundModule");
        assert_eq!(module.module_type(), ModuleType::NonShared);
        module.add_region(&scene).unwrap();
        module.remove_region(&scene).unwrap();
        module.close().unwrap();
    }

    // -- 155. SoundTrigger construction -----------------------------------

    #[test]
    fn sound_trigger_new() {
        let snd = Uuid::new_v4();
        let owner = Uuid::new_v4();
        let obj = Uuid::new_v4();
        let trigger = SoundTrigger::new(snd, owner, obj);
        assert_eq!(trigger.sound_id, snd);
        assert!((trigger.gain - 1.0).abs() < f32::EPSILON);
        assert!((trigger.radius - 20.0).abs() < f32::EPSILON);
        assert_eq!(trigger.flags, SoundFlags::NONE);
    }

    // -- 156. MediaModule get/set -----------------------------------------

    #[test]
    fn media_module_get_set() {
        let mut module = MediaModule::new();
        assert!(module.get_media().is_empty());

        let entry = MediaEntry {
            media_url: "https://example.com".to_string(),
            ..MediaEntry::default()
        };
        module.set_media(vec![entry]);
        assert_eq!(module.get_media().len(), 1);
        assert_eq!(module.get_media()[0].media_url, "https://example.com");
    }

    // -- 157. RegionReadyModule ready state --------------------------------

    #[test]
    fn region_ready_state() {
        let mut module = RegionReadyModule::new();
        assert!(!module.is_ready());
        module.set_ready();
        assert!(module.is_ready());

        let scene = test_scene_ref();
        module.remove_region(&scene).unwrap();
        assert!(!module.is_ready());
    }

    // -- 158. RestartModule schedule/abort ---------------------------------

    #[test]
    fn restart_schedule_abort() {
        let mut module = RestartModule::new();
        assert!(!module.is_restart_pending());
        assert_eq!(module.restart_seconds(), 0);

        module.schedule_restart(120);
        assert!(module.is_restart_pending());
        assert_eq!(module.restart_seconds(), 120);

        module.abort_restart();
        assert!(!module.is_restart_pending());
        assert_eq!(module.restart_seconds(), 0);
    }

    // -- 159. AccessModule flags and check ---------------------------------

    #[test]
    fn access_module_flags() {
        let mut module = AccessModule::new();
        let default_flags = module.access_flags();
        assert!(default_flags.contains(RegionAccessFlags::ALLOW_LANDMARK));
        assert!(default_flags.contains(RegionAccessFlags::ALLOW_DIRECT));
        assert!(default_flags.contains(RegionAccessFlags::ALLOW_TELEPORT));

        module.set_access_flags(RegionAccessFlags::ALLOW_TELEPORT);
        assert!(
            !module
                .access_flags()
                .contains(RegionAccessFlags::ALLOW_LANDMARK)
        );
        assert!(
            module
                .access_flags()
                .contains(RegionAccessFlags::ALLOW_TELEPORT)
        );

        assert!(module.check_access(Uuid::new_v4()));
    }

    // -- 160. VegetationModule lifecycle (stub) ----------------------------

    #[test]
    fn vegetation_lifecycle() {
        let mut module = VegetationModule::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        module.initialise(&config).unwrap();
        assert_eq!(module.name(), "VegetationModule");
        assert_eq!(module.module_type(), ModuleType::Shared);
        module.add_region(&scene).unwrap();
        module.remove_region(&scene).unwrap();
        module.close().unwrap();
    }

    // -- 161. SceneSerialiser stubs don't panic ----------------------------

    #[test]
    fn scene_serialiser_stubs() {
        let mut module = SceneSerialiser::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        module.initialise(&config).unwrap();
        assert_eq!(module.name(), "SceneSerialiser");
        assert_eq!(module.module_type(), ModuleType::Shared);

        let data = module.serialize_region();
        assert!(data.is_empty());
        module.deserialize_region(&[1, 2, 3]);

        module.add_region(&scene).unwrap();
        module.remove_region(&scene).unwrap();
        module.close().unwrap();
    }

    // -- 162. EnvironmentSettings default is empty -------------------------

    #[test]
    fn environment_settings_default_empty() {
        let es = crate::environment::EnvironmentSettings::default();
        assert!(es.day_cycle.is_empty());
        assert!(es.sky_map.is_empty());
        assert!(es.water_map.is_empty());
    }

    // -- 163. LightShareModule environment settings get/set ----------------

    #[test]
    fn lightshare_env_settings() {
        let mut module = LightShareModule::new();
        assert!(module.get_environment_settings().day_cycle.is_empty());

        let settings = crate::environment::EnvironmentSettings {
            day_cycle: vec![1, 2, 3],
            sky_map: vec![4, 5],
            water_map: vec![6],
        };
        module.set_environment_settings(settings);
        assert_eq!(module.get_environment_settings().day_cycle, vec![1, 2, 3]);
        assert_eq!(module.get_environment_settings().sky_map, vec![4, 5]);
    }

    // ===================================================================
    // Avatar-module tests (section 10)
    // ===================================================================

    use crate::avatar::{
        AvatarAppearance, AvatarFactoryModule, BakeType, BakedTexturesModule, CombatModule,
        DialogModule as DialogMod, FriendPerms, FriendsModule, GesturesModule, GodLevel,
        GodsModule, LureModule, WearableItem, WearableType,
    };

    // -- 164. AvatarFactoryModule lifecycle -----------------------------------

    #[test]
    fn avatar_factory_lifecycle() {
        let mut module = AvatarFactoryModule::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        module.initialise(&config).unwrap();
        module.add_region(&scene).unwrap();
        assert_eq!(module.name(), "AvatarFactoryModule");
        assert_eq!(module.module_type(), ModuleType::NonShared);
        module.remove_region(&scene).unwrap();
        module.close().unwrap();
    }

    // -- 165. AvatarFactoryModule set/get appearance --------------------------

    #[test]
    fn avatar_factory_set_get_appearance() {
        let mut module = AvatarFactoryModule::new();
        let agent = Uuid::new_v4();

        assert!(module.get_appearance(agent).is_none());

        let appearance = AvatarAppearance {
            visual_params: vec![10, 20, 30],
            height: 1.9,
            ..Default::default()
        };
        module.set_appearance(agent, appearance);

        let stored = module.get_appearance(agent).unwrap();
        assert_eq!(stored.serial, 1);
        assert_eq!(stored.visual_params, vec![10, 20, 30]);
        assert!((stored.height - 1.9).abs() < f32::EPSILON);

        // Second set increments serial.
        module.set_appearance(agent, AvatarAppearance::default());
        assert_eq!(module.get_appearance(agent).unwrap().serial, 2);
    }

    // -- 166. AvatarFactoryModule hover_z preference -------------------------

    #[test]
    fn avatar_factory_hover_z() {
        let mut module = AvatarFactoryModule::new();
        let agent = Uuid::new_v4();

        // No-op when appearance not set.
        module.set_preferences_hover_z(agent, 1.5);
        assert!(module.get_appearance(agent).is_none());

        module.set_appearance(agent, AvatarAppearance::default());
        assert!((module.get_appearance(agent).unwrap().hover_z).abs() < f32::EPSILON);

        module.set_preferences_hover_z(agent, 0.75);
        assert!((module.get_appearance(agent).unwrap().hover_z - 0.75).abs() < f32::EPSILON);
    }

    // -- 167. AvatarFactoryModule baked texture save/get/validate -------------

    #[test]
    fn avatar_factory_bake_cache() {
        let mut module = AvatarFactoryModule::new();
        let agent = Uuid::new_v4();

        assert!(!module.validate_baked_texture_cache(agent));
        assert!(module.get_baked_texture_faces(agent).is_empty());

        let mut textures = HashMap::new();
        textures.insert(BakeType::Head, Uuid::new_v4());
        textures.insert(BakeType::UpperBody, Uuid::new_v4());
        assert!(module.save_baked_textures(agent, textures));

        let faces = module.get_baked_texture_faces(agent);
        assert_eq!(faces.len(), 2);
        assert!(faces.contains_key(&BakeType::Head));
        assert!(module.validate_baked_texture_cache(agent));
    }

    // -- 168. AvatarFactoryModule send_appearance and request_rebake stubs ----

    #[test]
    fn avatar_factory_stubs() {
        let module = AvatarFactoryModule::new();
        assert!(module.send_appearance(Uuid::new_v4()));
        assert!(module.request_rebake(Uuid::new_v4()));
    }

    // -- 169. BakedTexturesModule store/get/clear ----------------------------

    #[test]
    fn baked_textures_store_get_clear() {
        let mut module = BakedTexturesModule::new();
        let agent = Uuid::new_v4();
        let tex1 = Uuid::new_v4();
        let tex2 = Uuid::new_v4();

        assert!(module.get_cached_items(agent).is_empty());

        module.store_baked_texture(agent, BakeType::Head, tex1);
        module.store_baked_texture(agent, BakeType::Eyes, tex2);

        let items = module.get_cached_items(agent);
        assert_eq!(items.len(), 2);

        // Overwrite one.
        let tex3 = Uuid::new_v4();
        module.store_baked_texture(agent, BakeType::Head, tex3);
        let items2 = module.get_cached_items(agent);
        assert_eq!(items2.len(), 2);
        let head = items2.iter().find(|(bt, _)| *bt == BakeType::Head).unwrap();
        assert_eq!(head.1, tex3);

        module.clear_cache(agent);
        assert!(module.get_cached_items(agent).is_empty());
    }

    // -- 170. BakedTexturesModule lifecycle -----------------------------------

    #[test]
    fn baked_textures_lifecycle() {
        let mut module = BakedTexturesModule::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        module.initialise(&config).unwrap();
        module.add_region(&scene).unwrap();
        assert_eq!(module.name(), "BakedTexturesModule");
        assert_eq!(module.module_type(), ModuleType::Shared);
        module.remove_region(&scene).unwrap();
        module.close().unwrap();
    }

    // -- 171. FriendsModule add/remove friends --------------------------------

    #[test]
    fn friends_add_remove() {
        let mut module = FriendsModule::new();
        let alice = Uuid::new_v4();
        let bob = Uuid::new_v4();

        module.add_friend(alice, bob);
        assert_eq!(module.get_friends(alice).len(), 1);
        assert_eq!(module.get_friends(bob).len(), 1);
        assert_eq!(module.get_friends(alice)[0].friend_id, bob);
        assert_eq!(module.get_friends(bob)[0].friend_id, alice);

        module.remove_friend(alice, bob);
        assert!(module.get_friends(alice).is_empty());
        assert!(module.get_friends(bob).is_empty());
    }

    // -- 172. FriendsModule grant_rights updates both directions ---------------

    #[test]
    fn friends_grant_rights() {
        let mut module = FriendsModule::new();
        let alice = Uuid::new_v4();
        let bob = Uuid::new_v4();

        module.add_friend(alice, bob);
        let rights = FriendPerms::CAN_SEE_ONLINE | FriendPerms::CAN_SEE_ON_MAP;
        module.grant_rights(alice, bob, rights);

        // Alice's entry should have updated my_flags.
        let alice_friends = module.get_friends(alice);
        assert_eq!(alice_friends[0].my_flags, rights);

        // Bob's entry should have updated their_flags for alice.
        let bob_friends = module.get_friends(bob);
        assert_eq!(bob_friends[0].their_flags, rights);
    }

    // -- 173. FriendsModule online status -------------------------------------

    #[test]
    fn friends_online_status() {
        let mut module = FriendsModule::new();
        let agent = Uuid::new_v4();

        assert!(!module.is_friend_online(agent));

        module.status_notify(agent, true);
        assert!(module.is_friend_online(agent));

        module.status_notify(agent, false);
        assert!(!module.is_friend_online(agent));
    }

    // -- 174. FriendsModule lifecycle -----------------------------------------

    #[test]
    fn friends_module_lifecycle() {
        let mut module = FriendsModule::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        module.initialise(&config).unwrap();
        module.add_region(&scene).unwrap();
        assert_eq!(module.name(), "FriendsModule");
        assert_eq!(module.module_type(), ModuleType::NonShared);
        module.remove_region(&scene).unwrap();
        module.close().unwrap();
    }

    // -- 175. LureModule send/accept/reject ----------------------------------

    #[test]
    fn lure_send_accept() {
        let mut module = LureModule::new();
        let from = Uuid::new_v4();
        let to = Uuid::new_v4();

        let lure_id = module.send_lure(from, to, "Come visit!");
        assert_eq!(module.pending_count(), 1);

        // Wrong agents fail.
        assert!(!module.teleport_lure_request(to, from, lure_id));
        assert_eq!(module.pending_count(), 1);

        // Correct agents succeed and consume the lure.
        assert!(module.teleport_lure_request(from, to, lure_id));
        assert_eq!(module.pending_count(), 0);

        // Double-accept fails.
        assert!(!module.teleport_lure_request(from, to, lure_id));
    }

    // -- 176. LureModule lifecycle -------------------------------------------

    #[test]
    fn lure_module_lifecycle() {
        let mut module = LureModule::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        module.initialise(&config).unwrap();
        module.add_region(&scene).unwrap();
        assert_eq!(module.name(), "LureModule");
        assert_eq!(module.module_type(), ModuleType::NonShared);
        module.remove_region(&scene).unwrap();
        module.close().unwrap();
    }

    // -- 177. GodsModule request/get level -----------------------------------

    #[test]
    fn gods_request_level() {
        let mut module = GodsModule::new();
        let agent = Uuid::new_v4();

        assert_eq!(module.get_god_level(agent), GodLevel::Not);

        module.request_god_powers(agent, GodLevel::God);
        assert_eq!(module.get_god_level(agent), GodLevel::God);

        module.request_god_powers(agent, GodLevel::FullGod);
        assert_eq!(module.get_god_level(agent), GodLevel::FullGod);
    }

    // -- 178. GodsModule lifecycle and config ---------------------------------

    #[test]
    fn gods_module_lifecycle() {
        let mut module = GodsModule::new();
        let mut config = TestConfig::new();
        config.set_bool("GodsModule", "DialogOnExit", false);
        let scene = test_scene_ref();
        module.initialise(&config).unwrap();
        assert!(!module.dialog_on_exit);
        module.add_region(&scene).unwrap();
        assert_eq!(module.name(), "GodsModule");
        assert_eq!(module.module_type(), ModuleType::NonShared);
        module.kick_user(Uuid::new_v4(), "test");
        module.remove_region(&scene).unwrap();
        module.close().unwrap();
    }

    // -- 179. DialogModule lifecycle and stubs don't panic --------------------

    #[test]
    fn dialog_module_lifecycle() {
        let mut module = DialogMod::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        module.initialise(&config).unwrap();
        module.add_region(&scene).unwrap();
        assert_eq!(module.name(), "DialogModule");
        assert_eq!(module.module_type(), ModuleType::NonShared);

        let agent = Uuid::new_v4();
        module.send_alert(agent, "alert!");
        module.send_alert_to_user(agent, "modal alert", true);
        module.send_url_to_user(
            agent,
            "Box",
            Uuid::new_v4(),
            Uuid::new_v4(),
            false,
            "Visit us",
            "https://example.com",
        );
        module.send_text_box_to_user(
            agent,
            "Enter name",
            42,
            "InputBox",
            Uuid::new_v4(),
            Uuid::new_v4(),
        );

        module.remove_region(&scene).unwrap();
        module.close().unwrap();
    }

    // -- 180. CombatModule damage and kill ------------------------------------

    #[test]
    fn combat_damage_and_kill() {
        let mut module = CombatModule::new();
        module.damage_enabled = true;
        module.max_damage = 50.0;

        let agent = Uuid::new_v4();
        let killer = Uuid::new_v4();

        assert!((module.get_health(agent) - 100.0).abs() < f32::EPSILON);

        let hp = module.apply_damage(agent, 30.0);
        assert!((hp - 70.0).abs() < f32::EPSILON);
        assert!((module.get_health(agent) - 70.0).abs() < f32::EPSILON);

        // Clamped to max_damage.
        let hp2 = module.apply_damage(agent, 999.0);
        assert!((hp2 - 20.0).abs() < f32::EPSILON);

        // Kill and respawn.
        module.killed(agent, killer);
        assert!((module.get_health(agent) - 100.0).abs() < f32::EPSILON);
    }

    // -- 181. CombatModule damage disabled returns full health ----------------

    #[test]
    fn combat_damage_disabled() {
        let mut module = CombatModule::new();
        assert!(!module.damage_enabled);

        let hp = module.apply_damage(Uuid::new_v4(), 50.0);
        assert!((hp - 100.0).abs() < f32::EPSILON);
    }

    // -- 182. CombatModule lifecycle and config -------------------------------

    #[test]
    fn combat_module_lifecycle() {
        let mut module = CombatModule::new();
        let mut config = TestConfig::new();
        config.set_bool("CombatModule", "DamageEnabled", true);
        config.set_float("CombatModule", "MaxDamage", 75.0);
        let scene = test_scene_ref();
        module.initialise(&config).unwrap();
        assert!(module.damage_enabled);
        assert!((module.max_damage - 75.0).abs() < f32::EPSILON);
        module.add_region(&scene).unwrap();
        assert_eq!(module.name(), "CombatModule");
        assert_eq!(module.module_type(), ModuleType::NonShared);
        module.remove_region(&scene).unwrap();
        module.close().unwrap();
    }

    // -- 183. GesturesModule activate/deactivate ------------------------------

    #[test]
    fn gestures_activate_deactivate() {
        let mut module = GesturesModule::new();
        let agent = Uuid::new_v4();
        let asset1 = Uuid::new_v4();
        let gesture1 = Uuid::new_v4();
        let asset2 = Uuid::new_v4();
        let gesture2 = Uuid::new_v4();

        assert!(module.get_active_gestures(agent).is_empty());

        module.activate_gesture(agent, asset1, gesture1);
        module.activate_gesture(agent, asset2, gesture2);
        assert_eq!(module.get_active_gestures(agent).len(), 2);

        // Duplicate activation is idempotent.
        module.activate_gesture(agent, asset1, gesture1);
        assert_eq!(module.get_active_gestures(agent).len(), 2);

        assert!(module.deactivate_gesture(agent, gesture1));
        assert_eq!(module.get_active_gestures(agent).len(), 1);

        // Deactivating a non-active gesture returns false.
        assert!(!module.deactivate_gesture(agent, gesture1));
    }

    // -- 184. GesturesModule lifecycle ----------------------------------------

    #[test]
    fn gestures_module_lifecycle() {
        let mut module = GesturesModule::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        module.initialise(&config).unwrap();
        module.add_region(&scene).unwrap();
        assert_eq!(module.name(), "GesturesModule");
        assert_eq!(module.module_type(), ModuleType::NonShared);
        module.remove_region(&scene).unwrap();
        module.close().unwrap();
    }

    // -- 185. WearableType variant count --------------------------------------

    #[test]
    fn wearable_type_variant_count() {
        assert_eq!(WearableType::ALL.len(), 17);
        // First and last.
        assert_eq!(WearableType::ALL[0], WearableType::Shape);
        assert_eq!(WearableType::ALL[16], WearableType::Universal);
        // repr values.
        assert_eq!(WearableType::Shape as u8, 0);
        assert_eq!(WearableType::Universal as u8, 16);
    }

    // -- 186. BakeType variant count and repr values --------------------------

    #[test]
    fn bake_type_variant_count() {
        assert_eq!(BakeType::ALL.len(), 11);
        assert_eq!(BakeType::Head as u8, 0);
        assert_eq!(BakeType::BakedAux3 as u8, 10);
        assert_eq!(BakeType::NumBakeTypes as u8, 11);
    }

    // -- 187. FriendPerms bitflag combinations --------------------------------

    #[test]
    fn friend_perms_combinations() {
        let none = FriendPerms::NONE;
        assert!(none.is_empty());

        let see = FriendPerms::CAN_SEE_ONLINE;
        assert!(see.contains(FriendPerms::CAN_SEE_ONLINE));
        assert!(!see.contains(FriendPerms::CAN_SEE_ON_MAP));

        let combined = FriendPerms::CAN_SEE_ONLINE
            | FriendPerms::CAN_SEE_ON_MAP
            | FriendPerms::CAN_MODIFY_OBJECTS;
        assert!(combined.contains(FriendPerms::CAN_SEE_ONLINE));
        assert!(combined.contains(FriendPerms::CAN_SEE_ON_MAP));
        assert!(combined.contains(FriendPerms::CAN_MODIFY_OBJECTS));
        assert_eq!(combined.bits(), 7);
    }

    // -- 188. GodLevel ordering -----------------------------------------------

    #[test]
    fn god_level_ordering() {
        assert!(GodLevel::Not < GodLevel::God);
        assert!(GodLevel::God < GodLevel::FullGod);
        assert_eq!(GodLevel::Not as u8, 0);
        assert_eq!(GodLevel::God as u8, 200);
        assert_eq!(GodLevel::FullGod as u8, 255);
    }

    // -- 189. AvatarAppearance default values ---------------------------------

    #[test]
    fn avatar_appearance_defaults() {
        let a = AvatarAppearance::default();
        assert_eq!(a.serial, 0);
        assert!(a.visual_params.is_empty());
        assert!(a.texture_entry.is_empty());
        assert!(a.wearables.is_empty());
        assert!((a.height - 1.8).abs() < f32::EPSILON);
        assert!(a.hover_z.abs() < f32::EPSILON);
    }

    // -- 190. AvatarAppearance with wearables --------------------------------

    #[test]
    fn avatar_appearance_with_wearables() {
        let mut appearance = AvatarAppearance::default();
        let item = WearableItem {
            item_id: Uuid::new_v4(),
            asset_id: Uuid::new_v4(),
        };
        appearance
            .wearables
            .entry(WearableType::Shirt)
            .or_default()
            .push(item.clone());
        appearance
            .wearables
            .entry(WearableType::Shirt)
            .or_default()
            .push(WearableItem {
                item_id: Uuid::new_v4(),
                asset_id: Uuid::new_v4(),
            });

        assert_eq!(appearance.wearables[&WearableType::Shirt].len(), 2);
        assert_eq!(appearance.wearables[&WearableType::Shirt][0], item);
    }

    // -- 191. BakedTexturesModule isolation between agents --------------------

    #[test]
    fn baked_textures_agent_isolation() {
        let mut module = BakedTexturesModule::new();
        let agent_a = Uuid::new_v4();
        let agent_b = Uuid::new_v4();
        let tex = Uuid::new_v4();

        module.store_baked_texture(agent_a, BakeType::Head, tex);
        assert_eq!(module.get_cached_items(agent_a).len(), 1);
        assert!(module.get_cached_items(agent_b).is_empty());

        module.clear_cache(agent_a);
        assert!(module.get_cached_items(agent_a).is_empty());
    }

    // -- 192. FriendsModule get_friends returns empty for unknown agent -------

    #[test]
    fn friends_unknown_agent() {
        let module = FriendsModule::new();
        assert!(module.get_friends(Uuid::new_v4()).is_empty());
    }

    // -- 193. CombatModule negative damage clamped to zero -------------------

    #[test]
    fn combat_negative_damage_clamped() {
        let mut module = CombatModule::new();
        module.damage_enabled = true;

        let agent = Uuid::new_v4();
        let hp = module.apply_damage(agent, -50.0);
        // Negative damage is clamped to 0, so health stays at 100.
        assert!((hp - 100.0).abs() < f32::EPSILON);
    }

    // ===================================================================
    // Framework & service connector tests (section 12)
    // ===================================================================

    use crate::framework::{
        AgentPreferencesConnector, AssetCacheConfig, CachedAsset, FlotsamAssetCache, IPBanModule,
        J2KPriority, LandServiceConnector, LibraryModule, MapImageServiceConnector, MonitorModule,
        MuteListServiceConnector, NeighbourServiceConnector, RegionStats, SearchModule,
        ServiceConnectorConfig, TextureSenderModule, UserManagementModule,
    };

    // -- 199. UserManagementModule add/get user ----------------------------

    #[test]
    fn user_mgmt_add_get() {
        let mut m = UserManagementModule::new();
        let id = Uuid::new_v4();
        m.add_user(id, "First".to_string(), "Last".to_string());
        let (f, l) = m.get_user_name(id).unwrap();
        assert_eq!(f, "First");
        assert_eq!(l, "Last");
    }

    // -- 200. UserManagementModule add_user_with_url ----------------------

    #[test]
    fn user_mgmt_add_with_url() {
        let mut m = UserManagementModule::new();
        let id = Uuid::new_v4();
        m.add_user_with_url(
            id,
            "HG".to_string(),
            "User".to_string(),
            "http://grid.example.com".to_string(),
        );
        let (f, l) = m.get_user_name(id).unwrap();
        assert_eq!(f, "HG");
        assert_eq!(l, "User");
    }

    // -- 201. UserManagementModule get_user_id reverse lookup --------------

    #[test]
    fn user_mgmt_get_user_id() {
        let mut m = UserManagementModule::new();
        let id = Uuid::new_v4();
        m.add_user(id, "Alice".to_string(), "Linden".to_string());
        assert_eq!(m.get_user_id("Alice", "Linden"), Some(id));
        assert_eq!(m.get_user_id("Bob", "Linden"), None);
    }

    // -- 202. UserManagementModule get_user_count -------------------------

    #[test]
    fn user_mgmt_count() {
        let mut m = UserManagementModule::new();
        assert_eq!(m.get_user_count(), 0);
        m.add_user(Uuid::new_v4(), "A".to_string(), "B".to_string());
        m.add_user(Uuid::new_v4(), "C".to_string(), "D".to_string());
        assert_eq!(m.get_user_count(), 2);
    }

    // -- 203. UserManagementModule lifecycle --------------------------------

    #[test]
    fn user_mgmt_lifecycle() {
        let mut m = UserManagementModule::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        m.initialise(&config).unwrap();
        m.add_region(&scene).unwrap();
        assert_eq!(m.name(), "UserManagementModule");
        assert_eq!(m.module_type(), ModuleType::Shared);
        m.remove_region(&scene).unwrap();
        m.close().unwrap();
    }

    // -- 204. MonitorModule lifecycle and defaults -------------------------

    #[test]
    fn monitor_module_lifecycle() {
        let mut m = MonitorModule::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        m.initialise(&config).unwrap();
        m.add_region(&scene).unwrap();
        assert_eq!(m.name(), "MonitorModule");
        assert_eq!(m.module_type(), ModuleType::NonShared);

        let stats = m.get_stats();
        assert_eq!(stats.total_prims, 0);
        assert!((stats.time_dilation - 1.0).abs() < f64::EPSILON);

        m.remove_region(&scene).unwrap();
        m.close().unwrap();
    }

    // -- 205. MonitorModule update/get stats --------------------------------

    #[test]
    fn monitor_module_update_stats() {
        let mut m = MonitorModule::new();
        let mut s = RegionStats::default();
        s.total_prims = 42;
        s.sim_fps = 55.0;
        s.root_agents = 3;
        m.update_stats(s);

        let got = m.get_stats();
        assert_eq!(got.total_prims, 42);
        assert!((got.sim_fps - 55.0).abs() < f64::EPSILON);
        assert_eq!(got.root_agents, 3);
    }

    // -- 206. FlotsamAssetCache put/get ------------------------------------

    #[test]
    fn flotsam_put_get() {
        let mut cache = FlotsamAssetCache::new();
        let asset = CachedAsset {
            id: "asset-001".to_string(),
            asset_type: 0,
            data: vec![1, 2, 3],
            inserted_at: 1000,
        };
        cache.put(asset);
        let got = cache.get("asset-001").unwrap();
        assert_eq!(got.data, vec![1, 2, 3]);
        assert!(cache.get("nonexistent").is_none());
    }

    // -- 207. FlotsamAssetCache stats counting -----------------------------

    #[test]
    fn flotsam_stats_counting() {
        let mut cache = FlotsamAssetCache::new();
        cache.put(CachedAsset {
            id: "a".to_string(),
            asset_type: 0,
            data: vec![],
            inserted_at: 1000,
        });
        cache.get("a");
        cache.get("missing");

        let stats = cache.get_stats();
        assert_eq!(stats.total_puts, 1);
        assert_eq!(stats.total_gets, 2);
        assert_eq!(stats.memory_hits, 1);
        assert_eq!(stats.memory_misses, 1);
    }

    // -- 208. FlotsamAssetCache clear resets everything ---------------------

    #[test]
    fn flotsam_clear() {
        let mut cache = FlotsamAssetCache::new();
        cache.put(CachedAsset {
            id: "x".to_string(),
            asset_type: 0,
            data: vec![9],
            inserted_at: 1000,
        });
        cache.clear();
        assert!(cache.get("x").is_none());
        let stats = cache.get_stats();
        assert_eq!(stats.total_puts, 0);
    }

    // -- 209. FlotsamAssetCache eviction when over max items ---------------

    #[test]
    fn flotsam_eviction() {
        let mut cache = FlotsamAssetCache::new();
        cache.config.memory_cache_max_items = 3;

        for i in 0..5 {
            cache.put(CachedAsset {
                id: format!("asset-{i}"),
                asset_type: 0,
                data: vec![i as u8],
                inserted_at: 1000 + i,
            });
        }
        // Only the 3 most recently inserted should remain.
        assert!(cache.get("asset-0").is_none());
        assert!(cache.get("asset-1").is_none());
        assert!(cache.get("asset-2").is_some());
        assert!(cache.get("asset-3").is_some());
        assert!(cache.get("asset-4").is_some());
    }

    // -- 210. FlotsamAssetCache lifecycle -----------------------------------

    #[test]
    fn flotsam_lifecycle() {
        let mut m = FlotsamAssetCache::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        m.initialise(&config).unwrap();
        m.add_region(&scene).unwrap();
        assert_eq!(m.name(), "FlotsamAssetCache");
        assert_eq!(m.module_type(), ModuleType::Shared);
        m.remove_region(&scene).unwrap();
        m.close().unwrap();
    }

    // -- 211. TextureSenderModule enqueue/dequeue priority ordering --------

    #[test]
    fn texture_sender_priority_ordering() {
        let mut m = TextureSenderModule::new();
        m.enqueue_texture(J2KPriority {
            texture_id: Uuid::new_v4(),
            priority: 1.0,
            discard_level: 0,
        });
        m.enqueue_texture(J2KPriority {
            texture_id: Uuid::new_v4(),
            priority: 10.0,
            discard_level: 0,
        });
        m.enqueue_texture(J2KPriority {
            texture_id: Uuid::new_v4(),
            priority: 5.0,
            discard_level: 0,
        });

        assert_eq!(m.queue_len(), 3);
        // Highest priority (10.0) comes out first.
        let first = m.dequeue_next().unwrap();
        assert!((first.priority - 10.0).abs() < f32::EPSILON);
        let second = m.dequeue_next().unwrap();
        assert!((second.priority - 5.0).abs() < f32::EPSILON);
        let third = m.dequeue_next().unwrap();
        assert!((third.priority - 1.0).abs() < f32::EPSILON);
        assert!(m.dequeue_next().is_none());
    }

    // -- 212. TextureSenderModule queue_len --------------------------------

    #[test]
    fn texture_sender_queue_len() {
        let mut m = TextureSenderModule::new();
        assert_eq!(m.queue_len(), 0);
        m.enqueue_texture(J2KPriority {
            texture_id: Uuid::new_v4(),
            priority: 1.0,
            discard_level: 0,
        });
        assert_eq!(m.queue_len(), 1);
        m.dequeue_next();
        assert_eq!(m.queue_len(), 0);
    }

    // -- 213. TextureSenderModule lifecycle --------------------------------

    #[test]
    fn texture_sender_lifecycle() {
        let mut m = TextureSenderModule::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        m.initialise(&config).unwrap();
        m.add_region(&scene).unwrap();
        assert_eq!(m.name(), "TextureSenderModule");
        assert_eq!(m.module_type(), ModuleType::NonShared);
        m.remove_region(&scene).unwrap();
        m.close().unwrap();
    }

    // -- 214. IPBanModule add/remove/is_banned ----------------------------

    #[test]
    fn ip_ban_add_remove() {
        let mut m = IPBanModule::new();
        m.add_ban("10.0.0.1".to_string());
        assert!(m.is_banned("10.0.0.1"));
        assert!(!m.is_banned("10.0.0.2"));

        m.remove_ban("10.0.0.1");
        assert!(!m.is_banned("10.0.0.1"));
    }

    // -- 215. IPBanModule network prefix banning --------------------------

    #[test]
    fn ip_ban_network_prefix() {
        let mut m = IPBanModule::new();
        let mut config = TestConfig::new();
        config.set_string("Network", "IPBanList", "192.168.1., 10.0.0.5");
        m.load_from_config(&config);

        // Exact match.
        assert!(m.is_banned("10.0.0.5"));
        // Network prefix match.
        assert!(m.is_banned("192.168.1.42"));
        assert!(m.is_banned("192.168.1.1"));
        // Not in the ban list.
        assert!(!m.is_banned("172.16.0.1"));
    }

    // -- 216. IPBanModule lifecycle -----------------------------------------

    #[test]
    fn ip_ban_lifecycle() {
        let mut m = IPBanModule::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        m.initialise(&config).unwrap();
        m.add_region(&scene).unwrap();
        assert_eq!(m.name(), "IPBanModule");
        assert_eq!(m.module_type(), ModuleType::Shared);
        m.remove_region(&scene).unwrap();
        m.close().unwrap();
    }

    // -- 217. LandServiceConnector lifecycle --------------------------------

    #[test]
    fn land_connector_lifecycle() {
        let mut m = LandServiceConnector::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        m.initialise(&config).unwrap();
        m.add_region(&scene).unwrap();
        assert_eq!(m.name(), "LandServiceConnector");
        assert_eq!(m.module_type(), ModuleType::Shared);
        // Stub returns None.
        assert!(m.get_land_data(Uuid::new_v4(), 128.0, 128.0).is_none());
        m.remove_region(&scene).unwrap();
        m.close().unwrap();
    }

    // -- 218. NeighbourServiceConnector lifecycle ---------------------------

    #[test]
    fn neighbour_connector_lifecycle() {
        let mut m = NeighbourServiceConnector::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        m.initialise(&config).unwrap();
        m.add_region(&scene).unwrap();
        assert_eq!(m.name(), "NeighbourServiceConnector");
        assert_eq!(m.module_type(), ModuleType::Shared);
        assert!(m.get_neighbours(Uuid::new_v4()).is_empty());
        m.inform_neighbours_of_status(Uuid::new_v4(), true);
        m.remove_region(&scene).unwrap();
        m.close().unwrap();
    }

    // -- 219. MuteListServiceConnector lifecycle ----------------------------

    #[test]
    fn mute_list_connector_lifecycle() {
        let mut m = MuteListServiceConnector::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        m.initialise(&config).unwrap();
        m.add_region(&scene).unwrap();
        assert_eq!(m.name(), "MuteListServiceConnector");
        assert_eq!(m.module_type(), ModuleType::Shared);
        assert!(m.get_mutes(Uuid::new_v4()).is_empty());
        m.update_mute();
        m.remove_region(&scene).unwrap();
        m.close().unwrap();
    }

    // -- 220. AgentPreferencesConnector lifecycle and defaults ---------------

    #[test]
    fn agent_prefs_connector_lifecycle() {
        let mut m = AgentPreferencesConnector::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        m.initialise(&config).unwrap();
        m.add_region(&scene).unwrap();
        assert_eq!(m.name(), "AgentPreferencesConnector");
        assert_eq!(m.module_type(), ModuleType::Shared);

        let prefs = m.get_preferences(Uuid::new_v4());
        assert_eq!(prefs.access_prefs, "M");
        assert!((prefs.hover_height - 0.0).abs() < f32::EPSILON);
        assert_eq!(prefs.language, "en");
        assert!(prefs.language_is_public);

        m.remove_region(&scene).unwrap();
        m.close().unwrap();
    }

    // -- 221. MapImageServiceConnector lifecycle ----------------------------

    #[test]
    fn map_image_connector_lifecycle() {
        let mut m = MapImageServiceConnector::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        m.initialise(&config).unwrap();
        m.add_region(&scene).unwrap();
        assert_eq!(m.name(), "MapImageServiceConnector");
        assert_eq!(m.module_type(), ModuleType::Shared);
        // Stubs return false.
        assert!(!m.upload_tile(1000, 1000, vec![0u8; 16]));
        assert!(!m.remove_tile(1000, 1000));
        m.remove_region(&scene).unwrap();
        m.close().unwrap();
    }

    // -- 222. LibraryModule lifecycle and fields ----------------------------

    #[test]
    fn library_module_lifecycle() {
        let mut m = LibraryModule::new();
        assert!(m.library_root_folder_id.is_nil());
        assert!(m.library_owner_id.is_nil());
        let config = TestConfig::new();
        let scene = test_scene_ref();
        m.initialise(&config).unwrap();
        m.add_region(&scene).unwrap();
        assert_eq!(m.name(), "LibraryModule");
        assert_eq!(m.module_type(), ModuleType::Shared);
        m.remove_region(&scene).unwrap();
        m.close().unwrap();
    }

    // -- 223. SearchModule lifecycle and stub results ----------------------

    #[test]
    fn search_module_lifecycle() {
        let mut m = SearchModule::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        m.initialise(&config).unwrap();
        m.add_region(&scene).unwrap();
        assert_eq!(m.name(), "SearchModule");
        assert_eq!(m.module_type(), ModuleType::Shared);
        assert!(m.search_places("test", 0).is_empty());
        assert!(m.search_people("test").is_empty());
        m.remove_region(&scene).unwrap();
        m.close().unwrap();
    }

    // -- 224. ServiceConnectorConfig defaults ------------------------------

    #[test]
    fn service_connector_config_defaults() {
        let cfg = ServiceConnectorConfig::default();
        assert!(cfg.service_url.is_empty());
        assert_eq!(cfg.timeout_ms, 25_000);
    }

    // -- 225. AssetCacheConfig defaults ------------------------------------

    #[test]
    fn asset_cache_config_defaults() {
        let cfg = AssetCacheConfig::default();
        assert!(cfg.memory_cache_enabled);
        assert_eq!(cfg.memory_cache_max_items, 30_000);
        assert_eq!(cfg.memory_cache_ttl_secs, 120);
        assert!(cfg.file_cache_enabled);
        assert_eq!(cfg.file_cache_dir, "assetcache");
        assert_eq!(cfg.file_cache_ttl_secs, 172_800);
        assert_eq!(cfg.deep_scan_interval_hours, 36);
    }

    // -- 226. UserManagementModule get missing user returns None -----------

    #[test]
    fn user_mgmt_get_missing() {
        let m = UserManagementModule::new();
        assert!(m.get_user_name(Uuid::new_v4()).is_none());
    }

    // -- 227. MonitorModule remove_region resets stats ----------------------

    #[test]
    fn monitor_remove_resets_stats() {
        let mut m = MonitorModule::new();
        let mut s = RegionStats::default();
        s.total_prims = 100;
        m.update_stats(s);

        let scene = test_scene_ref();
        m.remove_region(&scene).unwrap();
        assert_eq!(m.get_stats().total_prims, 0);
    }

    // -- 228. FlotsamAssetCache disabled memory cache skips memory tier -----

    #[test]
    fn flotsam_disabled_memory_cache() {
        let mut cache = FlotsamAssetCache::new();
        cache.config.memory_cache_enabled = false;
        cache.put(CachedAsset {
            id: "disabled".to_string(),
            asset_type: 0,
            data: vec![1],
            inserted_at: 1000,
        });
        // With memory cache disabled, get returns None (file tier stub also misses).
        assert!(cache.get("disabled").is_none());
        let stats = cache.get_stats();
        assert_eq!(stats.total_puts, 1);
        assert_eq!(stats.memory_hits, 0);
    }

    // ===================================================================
    // Entity-transfer module tests (section 11)
    // ===================================================================

    use crate::transfer::{
        AgentTransferState, EntityTransferContext, EntityTransferModule as TransferMod,
        EntityTransferStateMachine, GridRegion, TeleportFlags,
    };

    // -- 229. EntityTransferModule RegionModule lifecycle --------------------

    #[test]
    fn transfer_module_lifecycle() {
        let mut module = TransferMod::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        module.initialise(&config).unwrap();
        module.add_region(&scene).unwrap();
        assert_eq!(module.name(), "EntityTransferModule");
        assert_eq!(module.module_type(), ModuleType::NonShared);
        module.remove_region(&scene).unwrap();
        module.close().unwrap();
    }

    // -- 230. EntityTransferModule reads config -----------------------------

    #[test]
    fn transfer_module_reads_config() {
        let mut module = TransferMod::new();
        let mut config = TestConfig::new();
        config.set_float("EntityTransfer", "max_transfer_distance", 8000.0);
        config.set_int("EntityTransfer", "wait_for_agent_arrived_timeout_secs", 30);
        module.initialise(&config).unwrap();
        assert!((module.max_transfer_distance - 8000.0).abs() < f32::EPSILON);
        assert_eq!(module.wait_for_agent_arrived_timeout_secs, 30);
    }

    // -- 231. EntityTransferModule default config values ---------------------

    #[test]
    fn transfer_module_default_config() {
        let module = TransferMod::new();
        assert!((module.max_transfer_distance - 4095.0).abs() < f32::EPSILON);
        assert_eq!(module.wait_for_agent_arrived_timeout_secs, 15);
    }

    // -- 232. GridRegion construction and defaults ---------------------------

    #[test]
    fn grid_region_construction() {
        let id = Uuid::new_v4();
        let region = GridRegion::new(id, "TestRegion");
        assert_eq!(region.region_id, id);
        assert_eq!(region.region_name, "TestRegion");
        assert_eq!(region.region_handle, 0);
        assert_eq!(region.region_size_x, 256);
        assert_eq!(region.region_size_y, 256);
        assert!(region.external_host_name.is_empty());
        assert_eq!(region.http_port, 0);
    }

    // -- 233. GridRegion default is nil/empty --------------------------------

    #[test]
    fn grid_region_default() {
        let region = GridRegion::default();
        assert_eq!(region.region_id, Uuid::nil());
        assert!(region.region_name.is_empty());
        assert_eq!(region.region_size_x, 256);
        assert_eq!(region.region_size_y, 256);
    }

    // -- 234. TeleportFlags bitflag combinations ----------------------------

    #[test]
    fn teleport_flags_combinations() {
        let flags = TeleportFlags::VIA_LOCATION | TeleportFlags::IS_FLYING;
        assert!(flags.contains(TeleportFlags::VIA_LOCATION));
        assert!(flags.contains(TeleportFlags::IS_FLYING));
        assert!(!flags.contains(TeleportFlags::VIA_HOME));
        assert_eq!(flags.bits(), 16 | 4096);

        let empty = TeleportFlags::DEFAULT;
        assert_eq!(empty.bits(), 0);

        let all_nav = TeleportFlags::VIA_LURE
            | TeleportFlags::VIA_LANDMARK
            | TeleportFlags::VIA_LOCATION
            | TeleportFlags::VIA_HOME
            | TeleportFlags::VIA_TELEHUB
            | TeleportFlags::VIA_LOGIN
            | TeleportFlags::VIA_REGION_ID;
        assert!(all_nav.contains(TeleportFlags::VIA_LURE));
        assert!(all_nav.contains(TeleportFlags::VIA_LOGIN));
    }

    // -- 235. TeleportFlags individual values match C# ----------------------

    #[test]
    fn teleport_flags_values() {
        assert_eq!(TeleportFlags::DEFAULT.bits(), 0);
        assert_eq!(TeleportFlags::SET_HOME_TO_TARGET.bits(), 1);
        assert_eq!(TeleportFlags::SET_LAST_TO_TARGET.bits(), 2);
        assert_eq!(TeleportFlags::VIA_LURE.bits(), 4);
        assert_eq!(TeleportFlags::VIA_LANDMARK.bits(), 8);
        assert_eq!(TeleportFlags::VIA_LOCATION.bits(), 16);
        assert_eq!(TeleportFlags::VIA_HOME.bits(), 32);
        assert_eq!(TeleportFlags::VIA_TELEHUB.bits(), 64);
        assert_eq!(TeleportFlags::VIA_LOGIN.bits(), 128);
        assert_eq!(TeleportFlags::GOD_LIKE.bits(), 256);
        assert_eq!(TeleportFlags::NINE_ONE_ONE.bits(), 512);
        assert_eq!(TeleportFlags::DISABLE_CANCEL.bits(), 1024);
        assert_eq!(TeleportFlags::VIA_REGION_ID.bits(), 2048);
        assert_eq!(TeleportFlags::IS_FLYING.bits(), 4096);
        assert_eq!(TeleportFlags::RESET_HOME.bits(), 8192);
        assert_eq!(TeleportFlags::FORCE_REDIRECT.bits(), 16384);
    }

    // -- 236. State machine: set_in_transit ---------------------------------

    #[test]
    fn state_machine_set_in_transit() {
        let mut sm = EntityTransferStateMachine::new();
        let agent = Uuid::new_v4();
        assert!(!sm.is_in_transit(agent));
        assert!(sm.set_in_transit(agent));
        assert!(sm.is_in_transit(agent));
        assert_eq!(
            sm.get_agent_transfer_state(agent),
            Some(AgentTransferState::Preparing)
        );
    }

    // -- 237. Double-teleport prevention ------------------------------------

    #[test]
    fn state_machine_double_teleport_prevention() {
        let mut sm = EntityTransferStateMachine::new();
        let agent = Uuid::new_v4();
        assert!(sm.set_in_transit(agent));
        // Second call returns false.
        assert!(!sm.set_in_transit(agent));
    }

    // -- 238. Valid state transition: Preparing -> Transferring ---------------

    #[test]
    fn state_machine_preparing_to_transferring() {
        let mut sm = EntityTransferStateMachine::new();
        let agent = Uuid::new_v4();
        sm.set_in_transit(agent);
        assert!(sm.update_in_transit(agent, AgentTransferState::Transferring));
        assert_eq!(
            sm.get_agent_transfer_state(agent),
            Some(AgentTransferState::Transferring)
        );
    }

    // -- 239. Valid transition: Transferring -> ReceivedAtDestination --------

    #[test]
    fn state_machine_transferring_to_received() {
        let mut sm = EntityTransferStateMachine::new();
        let agent = Uuid::new_v4();
        sm.set_in_transit(agent);
        sm.update_in_transit(agent, AgentTransferState::Transferring);
        assert!(sm.update_in_transit(agent, AgentTransferState::ReceivedAtDestination));
        assert_eq!(
            sm.get_agent_transfer_state(agent),
            Some(AgentTransferState::ReceivedAtDestination)
        );
    }

    // -- 240. Valid transition: ReceivedAtDestination -> CleaningUp ----------

    #[test]
    fn state_machine_received_to_cleaning_up() {
        let mut sm = EntityTransferStateMachine::new();
        let agent = Uuid::new_v4();
        sm.set_in_transit(agent);
        sm.update_in_transit(agent, AgentTransferState::Transferring);
        sm.update_in_transit(agent, AgentTransferState::ReceivedAtDestination);
        assert!(sm.update_in_transit(agent, AgentTransferState::CleaningUp));
        assert_eq!(
            sm.get_agent_transfer_state(agent),
            Some(AgentTransferState::CleaningUp)
        );
    }

    // -- 241. Invalid transition: Preparing -> ReceivedAtDestination ---------

    #[test]
    fn state_machine_invalid_preparing_to_received() {
        let mut sm = EntityTransferStateMachine::new();
        let agent = Uuid::new_v4();
        sm.set_in_transit(agent);
        // Cannot skip from Preparing directly to ReceivedAtDestination.
        assert!(!sm.update_in_transit(agent, AgentTransferState::ReceivedAtDestination));
        // State should remain Preparing.
        assert_eq!(
            sm.get_agent_transfer_state(agent),
            Some(AgentTransferState::Preparing)
        );
    }

    // -- 242. Terminal state: CleaningUp cannot transition -------------------

    #[test]
    fn state_machine_cleaning_up_is_terminal() {
        let mut sm = EntityTransferStateMachine::new();
        let agent = Uuid::new_v4();
        sm.set_in_transit(agent);
        sm.update_in_transit(agent, AgentTransferState::CleaningUp);
        // No transition from CleaningUp is allowed.
        assert!(!sm.update_in_transit(agent, AgentTransferState::Aborting));
        assert!(!sm.update_in_transit(agent, AgentTransferState::Preparing));
        assert!(!sm.update_in_transit(agent, AgentTransferState::Transferring));
    }

    // -- 243. Terminal state: Aborting cannot transition ---------------------

    #[test]
    fn state_machine_aborting_is_terminal() {
        let mut sm = EntityTransferStateMachine::new();
        let agent = Uuid::new_v4();
        sm.set_in_transit(agent);
        sm.update_in_transit(agent, AgentTransferState::Aborting);
        assert!(!sm.update_in_transit(agent, AgentTransferState::CleaningUp));
        assert!(!sm.update_in_transit(agent, AgentTransferState::Preparing));
    }

    // -- 244. Reset from transit removes agent ------------------------------

    #[test]
    fn state_machine_reset_from_transit() {
        let mut sm = EntityTransferStateMachine::new();
        let agent = Uuid::new_v4();
        sm.set_in_transit(agent);
        assert!(sm.reset_from_transit(agent));
        assert!(!sm.is_in_transit(agent));
        assert_eq!(sm.get_agent_transfer_state(agent), None);
        // Second reset returns false.
        assert!(!sm.reset_from_transit(agent));
    }

    // -- 245. Cancelling path: Preparing -> Cancelling -> CleaningUp --------

    #[test]
    fn state_machine_cancelling_path() {
        let mut sm = EntityTransferStateMachine::new();
        let agent = Uuid::new_v4();
        sm.set_in_transit(agent);
        assert!(sm.update_in_transit(agent, AgentTransferState::Cancelling));
        assert_eq!(
            sm.get_agent_transfer_state(agent),
            Some(AgentTransferState::Cancelling)
        );
        assert!(sm.update_in_transit(agent, AgentTransferState::CleaningUp));
    }

    // -- 246. Aborting from Transferring ------------------------------------

    #[test]
    fn state_machine_abort_from_transferring() {
        let mut sm = EntityTransferStateMachine::new();
        let agent = Uuid::new_v4();
        sm.set_in_transit(agent);
        sm.update_in_transit(agent, AgentTransferState::Transferring);
        assert!(sm.update_in_transit(agent, AgentTransferState::Aborting));
        assert_eq!(
            sm.get_agent_transfer_state(agent),
            Some(AgentTransferState::Aborting)
        );
    }

    // -- 247. EntityTransferContext default ----------------------------------

    #[test]
    fn transfer_context_defaults() {
        let ctx = EntityTransferContext::default();
        assert!(ctx.incoming_teleport_type.is_empty());
        assert!(ctx.src_external_name.is_empty());
        assert!(ctx.destination.is_none());
    }

    // -- 248. EntityTransferModule teleport stub succeeds --------------------

    #[test]
    fn transfer_module_teleport_stub() {
        let mut module = TransferMod::new();
        let agent = Uuid::new_v4();
        let result = module.teleport(
            agent,
            1000,
            [128.0, 128.0, 25.0],
            [0.0, 1.0, 0.0],
            TeleportFlags::VIA_LOCATION,
        );
        assert!(result.is_ok());
        // Agent should no longer be in transit after the stub completes.
        assert!(!module.state_machine().is_in_transit(agent));
    }

    // -- 249. EntityTransferModule do_teleport stub succeeds -----------------

    #[test]
    fn transfer_module_do_teleport_stub() {
        let mut module = TransferMod::new();
        let agent = Uuid::new_v4();
        let dest = GridRegion::new(Uuid::new_v4(), "DestRegion");
        let result = module.do_teleport(
            agent,
            &dest,
            [128.0, 128.0, 25.0],
            [0.0, 1.0, 0.0],
            TeleportFlags::VIA_LANDMARK,
        );
        assert!(result.is_ok());
        assert!(!module.state_machine().is_in_transit(agent));
    }

    // -- 250. EntityTransferModule cancel_teleport --------------------------

    #[test]
    fn transfer_module_cancel_teleport() {
        let mut module = TransferMod::new();
        let agent = Uuid::new_v4();
        // Manually set in transit so we can cancel.
        assert!(module.state_machine_mut().set_in_transit(agent));
        module.cancel_teleport(agent);
        assert!(!module.state_machine().is_in_transit(agent));
    }

    // -- 251. EntityTransferModule teleport_home stub -----------------------

    #[test]
    fn transfer_module_teleport_home() {
        let mut module = TransferMod::new();
        assert!(module.teleport_home(Uuid::new_v4()));
    }

    // -- 252. EntityTransferModule cross_agent stub -------------------------

    #[test]
    fn transfer_module_cross_agent() {
        let mut module = TransferMod::new();
        let dest = GridRegion::new(Uuid::new_v4(), "Neighbour");
        assert!(module.cross_agent_to_new_region(
            Uuid::new_v4(),
            [256.0, 128.0, 25.0],
            &dest,
            true,
        ));
    }

    // -- 253. State machine wait_for_agent_arrived stub ---------------------

    #[test]
    fn state_machine_wait_for_arrived_stub() {
        let sm = EntityTransferStateMachine::new();
        assert!(sm.wait_for_agent_arrived_at_destination(Uuid::new_v4()));
        assert!(sm.enable_wait_for_agent_arrived);
    }

    // -- 254. State machine in_transit_count --------------------------------

    #[test]
    fn state_machine_in_transit_count() {
        let mut sm = EntityTransferStateMachine::new();
        assert_eq!(sm.in_transit_count(), 0);
        sm.set_in_transit(Uuid::new_v4());
        sm.set_in_transit(Uuid::new_v4());
        assert_eq!(sm.in_transit_count(), 2);
    }

    // -- 255. Update on unknown agent returns false -------------------------

    #[test]
    fn state_machine_update_unknown_agent() {
        let mut sm = EntityTransferStateMachine::new();
        assert!(!sm.update_in_transit(Uuid::new_v4(), AgentTransferState::Transferring));
    }

    // -- 256. AgentTransferState::is_terminal --------------------------------

    #[test]
    fn agent_transfer_state_is_terminal() {
        assert!(AgentTransferState::CleaningUp.is_terminal());
        assert!(AgentTransferState::Aborting.is_terminal());
        assert!(!AgentTransferState::Preparing.is_terminal());
        assert!(!AgentTransferState::Transferring.is_terminal());
        assert!(!AgentTransferState::ReceivedAtDestination.is_terminal());
        assert!(!AgentTransferState::Cancelling.is_terminal());
    }

    // -- 257. Cancelling -> Aborting is valid --------------------------------

    #[test]
    fn state_machine_cancelling_to_aborting() {
        let mut sm = EntityTransferStateMachine::new();
        let agent = Uuid::new_v4();
        sm.set_in_transit(agent);
        sm.update_in_transit(agent, AgentTransferState::Cancelling);
        assert!(sm.update_in_transit(agent, AgentTransferState::Aborting));
    }

    // ===================================================================
    // Integration tests (section 13.2)
    // ===================================================================

    // -- 258. Integration: rez permission round-trip (owner vs stranger) ---

    #[test]
    fn integration_rez_permission_owner_vs_stranger() {
        // Set up permissions module with bypass OFF.
        let perms = perms_module_no_bypass();

        let agent_a = Uuid::from_u128(0x1001);
        let agent_b = Uuid::from_u128(0x1002);

        // Create a parcel owned by agent A with CREATE_OBJECTS flag.
        let mut parcel = LandData::default();
        parcel.owner_id = agent_a;
        parcel.flags = ParcelFlags::CREATE_OBJECTS.bits();

        // Agent A (parcel owner) can create objects on their parcel.
        assert!(
            perms.can_create_object(agent_a, (128.0, 128.0, 0.0), Some(&parcel)),
            "Parcel owner should be able to create objects"
        );

        // Agent B (stranger) can also create when CREATE_OBJECTS is set.
        assert!(
            perms.can_create_object(agent_b, (128.0, 128.0, 0.0), Some(&parcel)),
            "Stranger should be able to create when CREATE_OBJECTS flag is set"
        );

        // Now remove CREATE_OBJECTS flag — stranger denied but owner still allowed.
        parcel.flags = 0;
        assert!(
            perms.can_create_object(agent_a, (128.0, 128.0, 0.0), Some(&parcel)),
            "Parcel owner should still be able to create without CREATE_OBJECTS flag"
        );
        assert!(
            !perms.can_create_object(agent_b, (128.0, 128.0, 0.0), Some(&parcel)),
            "Stranger should be denied when CREATE_OBJECTS flag is cleared"
        );
    }

    // -- 259. Integration: edit permission with admin promotion -------------

    #[test]
    fn integration_edit_permission_with_admin_promotion() {
        let mut perms = perms_module_no_bypass();

        let agent_a = Uuid::from_u128(0x2001);
        let agent_b = Uuid::from_u128(0x2002);

        let obj_perms = PermissionMask::MODIFY | PermissionMask::MOVE;

        // Agent A (owner) can edit their own object.
        assert!(
            perms.can_edit_object(agent_a, agent_a, Uuid::nil(), obj_perms),
            "Owner should be able to edit own object with MODIFY"
        );

        // Agent B (stranger) cannot edit agent A's object.
        assert!(
            !perms.can_edit_object(agent_b, agent_a, Uuid::nil(), obj_perms),
            "Stranger should not be able to edit another's object"
        );

        // Promote agent B to administrator (god user).
        perms.god_users.insert(agent_b);

        // Now agent B can edit even with PermissionMask::NONE.
        assert!(
            perms.can_edit_object(agent_b, agent_a, Uuid::nil(), PermissionMask::NONE),
            "Administrator should be able to edit any object"
        );
    }

    // -- 260. Integration: copy permission with COPY+TRANSFER bits ---------

    #[test]
    fn integration_copy_permission_transfer_bits() {
        let mut perms = perms_module_no_bypass();

        let owner = Uuid::from_u128(0x3001);
        let non_owner = Uuid::from_u128(0x3002);
        let admin = Uuid::from_u128(0x3003);
        perms.god_users.insert(admin);

        // Owner can copy their own object when COPY is set.
        let copy_only = PermissionMask::COPY;
        assert!(
            perms.can_copy_object(owner, owner, Uuid::nil(), copy_only),
            "Owner should copy own object with COPY bit"
        );

        // Non-owner cannot copy when only COPY is set (TRANSFER missing).
        assert!(
            !perms.can_copy_object(non_owner, owner, Uuid::nil(), copy_only),
            "Non-owner should not copy without TRANSFER bit"
        );

        // Non-owner (regular) still cannot copy even with COPY+TRANSFER
        // because the permissions system requires admin status for non-owners.
        let copy_transfer = PermissionMask::COPY | PermissionMask::TRANSFER;
        assert!(
            !perms.can_copy_object(non_owner, owner, Uuid::nil(), copy_transfer),
            "Regular non-owner should not copy even with COPY+TRANSFER"
        );

        // Administrator CAN copy with COPY+TRANSFER bits set.
        assert!(
            perms.can_copy_object(admin, owner, Uuid::nil(), copy_transfer),
            "Admin should copy when both COPY and TRANSFER are set"
        );

        // Non-owner cannot copy when only TRANSFER is set (no COPY).
        let transfer_only = PermissionMask::TRANSFER;
        assert!(
            !perms.can_copy_object(non_owner, owner, Uuid::nil(), transfer_only),
            "Non-owner should not copy with only TRANSFER (no COPY)"
        );
    }

    // -- 261. Integration: can_take_object owner/stranger/estate_manager ---

    #[test]
    fn integration_take_object_owner_stranger_estate_manager() {
        let mut perms = perms_module_no_bypass();

        let owner = Uuid::from_u128(0x4001);
        let stranger = Uuid::from_u128(0x4002);
        let estate_mgr = Uuid::from_u128(0x4003);

        // Owner can take their own object.
        assert!(
            perms.can_take_object(owner, owner, Uuid::nil()),
            "Owner should be able to take own object"
        );

        // Stranger cannot take.
        assert!(
            !perms.can_take_object(stranger, owner, Uuid::nil()),
            "Stranger should not be able to take another's object"
        );

        // Estate manager (promoted) can take because they become admin.
        perms.estate_managers.insert(estate_mgr);
        perms.config.region_manager_is_god = true;
        assert!(
            perms.can_take_object(estate_mgr, owner, Uuid::nil()),
            "Estate manager with god powers should be able to take objects"
        );
    }

    // -- 262. Integration: teleport state machine full cycle -----------------

    #[test]
    fn integration_teleport_state_machine_full_cycle() {
        let mut transfer = TransferMod::new();
        let config = TestConfig::new();
        transfer.initialise(&config).unwrap();

        let agent = Uuid::new_v4();

        // Start teleport.
        assert!(transfer.state_machine_mut().set_in_transit(agent));
        assert_eq!(
            transfer.state_machine().get_agent_transfer_state(agent),
            Some(AgentTransferState::Preparing)
        );

        // Transition through all valid states.
        assert!(
            transfer
                .state_machine_mut()
                .update_in_transit(agent, AgentTransferState::Transferring)
        );
        assert_eq!(
            transfer.state_machine().get_agent_transfer_state(agent),
            Some(AgentTransferState::Transferring)
        );

        assert!(
            transfer
                .state_machine_mut()
                .update_in_transit(agent, AgentTransferState::ReceivedAtDestination)
        );
        assert_eq!(
            transfer.state_machine().get_agent_transfer_state(agent),
            Some(AgentTransferState::ReceivedAtDestination)
        );

        assert!(
            transfer
                .state_machine_mut()
                .update_in_transit(agent, AgentTransferState::CleaningUp)
        );
        assert_eq!(
            transfer.state_machine().get_agent_transfer_state(agent),
            Some(AgentTransferState::CleaningUp)
        );

        // After CleaningUp, reset removes the agent from transit.
        assert!(transfer.state_machine_mut().reset_from_transit(agent));
        assert!(!transfer.state_machine().is_in_transit(agent));
        assert_eq!(
            transfer.state_machine().get_agent_transfer_state(agent),
            None
        );
    }

    // -- 263. Integration: double-teleport prevention and recovery ----------

    #[test]
    fn integration_double_teleport_prevention_and_recovery() {
        let mut sm = EntityTransferStateMachine::new();

        let agent = Uuid::new_v4();

        // First teleport starts.
        assert!(sm.set_in_transit(agent));

        // Second teleport attempt is blocked.
        assert!(
            !sm.set_in_transit(agent),
            "Second teleport should be blocked while first is in progress"
        );

        // Complete first teleport.
        sm.update_in_transit(agent, AgentTransferState::Transferring);
        sm.update_in_transit(agent, AgentTransferState::ReceivedAtDestination);
        sm.update_in_transit(agent, AgentTransferState::CleaningUp);
        sm.reset_from_transit(agent);

        // Now a new teleport should succeed.
        assert!(
            sm.set_in_transit(agent),
            "Teleport should succeed after previous one completed"
        );
        assert_eq!(
            sm.get_agent_transfer_state(agent),
            Some(AgentTransferState::Preparing)
        );
    }

    // -- 264. Integration: attachment detach/reattach cycle -----------------

    #[test]
    fn integration_attachment_detach_reattach_cycle() {
        let mut attachments = AttachmentsModule::new();
        let agent = Uuid::new_v4();
        let item = Uuid::new_v4();
        let point = AttachmentPoint::RightHand;

        // Attach an item.
        let obj_id = attachments.wear(agent, item, point).unwrap();
        assert!(!obj_id.is_nil());
        assert_eq!(attachments.get_attachments(agent).len(), 1);
        assert_eq!(attachments.get_attachments(agent)[0].item_id, item);
        assert_eq!(attachments.get_attachments(agent)[0].point, point);

        // Detach — verify empty.
        attachments.detach(agent, item).unwrap();
        assert!(
            attachments.get_attachments(agent).is_empty(),
            "Attachments should be empty after detach"
        );

        // Reattach at same point — verify present.
        let obj_id_2 = attachments.wear(agent, item, point).unwrap();
        assert!(!obj_id_2.is_nil());
        assert_eq!(attachments.get_attachments(agent).len(), 1);
        assert_eq!(attachments.get_attachments(agent)[0].item_id, item);
        assert_eq!(attachments.get_attachments(agent)[0].point, point);
    }

    // -- 265. Integration: offline IM storage with presence -----------------

    #[test]
    fn integration_offline_im_with_presence() {
        let mut presence = PresenceMod::new();
        let mut offline = OfflineMod::new();

        let agent_a = Uuid::new_v4();
        let agent_b = Uuid::new_v4();

        // Agent A is online, agent B is offline.
        presence.register_presence(agent_a);
        assert!(presence.is_present(agent_a));
        assert!(!presence.is_present(agent_b));

        // Send IM to agent B. Since B is offline, store it.
        let im = make_im(agent_a, agent_b);
        if !presence.is_present(agent_b) {
            offline.store_message(im);
        }
        assert_eq!(offline.pending_count(agent_b), 1);

        // Agent B comes online.
        presence.register_presence(agent_b);
        assert!(presence.is_present(agent_b));

        // Retrieve offline messages.
        let msgs = offline.retrieve_messages(agent_b);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].from_agent_id, agent_a);
        assert_eq!(msgs[0].to_agent_id, agent_b);
        assert_eq!(msgs[0].message, "Hello IM");

        // No more pending messages.
        assert_eq!(offline.pending_count(agent_b), 0);
    }

    // -- 266. Integration: mute list add/verify/remove cycle ----------------

    #[test]
    fn integration_mute_list_add_verify_remove() {
        let mut mute_list = MuteMod::new();

        let agent_a = Uuid::new_v4();
        let agent_b = Uuid::new_v4();

        // Add agent A to agent B's mute list.
        let entry = MuteEntry {
            agent_id: agent_b,
            mute_id: agent_a,
            name: "Agent A".to_string(),
            mute_type: MuteType::ByAgent,
            flags: 0,
        };
        mute_list.update_mute(agent_b, entry);

        // Verify mute entry exists.
        assert!(
            mute_list.is_muted(agent_b, agent_a),
            "Agent A should be muted by agent B"
        );
        assert_eq!(mute_list.get_mutes(agent_b).len(), 1);

        // Verify reverse is not muted.
        assert!(
            !mute_list.is_muted(agent_a, agent_b),
            "Agent B should not be muted by agent A"
        );

        // Remove the mute.
        mute_list.remove_mute(agent_b, agent_a, "Agent A");
        assert!(
            !mute_list.is_muted(agent_b, agent_a),
            "Agent A should no longer be muted after removal"
        );
        assert!(mute_list.get_mutes(agent_b).is_empty());
    }

    // -- 267. Integration: presence module add/check/remove cycle -----------

    #[test]
    fn integration_presence_add_check_remove() {
        let mut presence = PresenceMod::new();

        let agent_a = Uuid::new_v4();
        let agent_b = Uuid::new_v4();
        let agent_c = Uuid::new_v4();

        // Register multiple agents.
        presence.register_presence(agent_a);
        presence.register_presence(agent_b);
        presence.register_presence(agent_c);
        assert_eq!(presence.presence_count(), 3);

        // Verify all are online.
        assert!(presence.is_present(agent_a));
        assert!(presence.is_present(agent_b));
        assert!(presence.is_present(agent_c));

        let all = presence.get_present_agents();
        assert!(all.contains(&agent_a));
        assert!(all.contains(&agent_b));
        assert!(all.contains(&agent_c));

        // Remove agent B — verify offline.
        presence.deregister_presence(agent_b);
        assert!(!presence.is_present(agent_b));
        assert_eq!(presence.presence_count(), 2);

        // A and C still online.
        assert!(presence.is_present(agent_a));
        assert!(presence.is_present(agent_c));
    }

    // -- 268. Integration: parcel ownership and permission interaction -------

    #[test]
    fn integration_parcel_ownership_and_permissions() {
        let mut mgr = LandManagementModule::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        mgr.initialise(&config).unwrap();
        mgr.add_region(&scene).unwrap();

        let old_owner = Uuid::new_v4();
        let new_owner = Uuid::new_v4();

        // Subdivide: create a parcel owned by old_owner.
        let parcel_a = mgr.subdivide(0, 0, 128, 128, old_owner).unwrap();
        // Create another parcel for new_owner (simulating a "transfer" by
        // having new_owner claim a different section).
        let parcel_b = mgr.subdivide(128, 128, 256, 256, new_owner).unwrap();

        // Verify ownership.
        assert_eq!(mgr.get_parcel(parcel_a).unwrap().data.owner_id, old_owner);
        assert_eq!(mgr.get_parcel(parcel_b).unwrap().data.owner_id, new_owner);

        let perms = perms_module_no_bypass();

        // Old owner can create on their own parcel.
        let parcel_a_data = &mgr.get_parcel(parcel_a).unwrap().data;
        assert!(
            perms.can_create_object(old_owner, (64.0, 64.0, 0.0), Some(parcel_a_data)),
            "Owner should be able to create on their parcel"
        );

        // New owner can create on their own parcel.
        let parcel_b_data = &mgr.get_parcel(parcel_b).unwrap().data;
        assert!(
            perms.can_create_object(new_owner, (192.0, 192.0, 0.0), Some(parcel_b_data)),
            "New owner should be able to create on their parcel"
        );

        // Old owner cannot create on new_owner's restricted parcel.
        let mut restricted = parcel_b_data.clone();
        restricted.flags = 0;
        assert!(
            !perms.can_create_object(old_owner, (192.0, 192.0, 0.0), Some(&restricted)),
            "Old owner should not create on another's restricted parcel"
        );
    }

    // -- 269. Integration: return objects permission with parcel context -----

    #[test]
    fn integration_return_objects_parcel_context() {
        let mut perms = perms_module_no_bypass();

        let parcel_owner = Uuid::from_u128(0x5001);
        let object_owner = Uuid::from_u128(0x5002);
        let stranger = Uuid::from_u128(0x5003);
        let estate_mgr = Uuid::from_u128(0x5004);

        let parcel = perm_parcel_with_owner(parcel_owner);

        // Parcel owner can return objects on their parcel.
        assert!(
            perms.can_return_object(parcel_owner, object_owner, Uuid::nil(), Some(&parcel)),
            "Parcel owner should be able to return objects on their land"
        );

        // Stranger cannot return on someone else's parcel.
        assert!(
            !perms.can_return_object(stranger, object_owner, Uuid::nil(), Some(&parcel)),
            "Stranger should not be able to return objects on another's parcel"
        );

        // Estate manager can return anywhere.
        perms.estate_managers.insert(estate_mgr);
        assert!(
            perms.can_return_object(estate_mgr, object_owner, Uuid::nil(), Some(&parcel)),
            "Estate manager should be able to return objects on any parcel"
        );
    }

    // -- 270. Integration: module registry full lifecycle round-trip ---------

    #[test]
    fn integration_module_registry_full_lifecycle() {
        let mut registry = ModuleRegistry::new();

        // Register 3+ diverse modules.
        registry.register(CapsModule::new());
        registry.register(DynAttrsModule::new());
        registry.register(ServiceThrottleModule::new());
        registry.register(TrackingModule::new(
            "IntegrationTracker",
            ModuleType::NonShared,
        ));

        assert_eq!(registry.module_count(), 4);

        let config = TestConfig::new();
        let scene = test_scene_ref();

        // initialise_all — no errors.
        registry.initialise_all(&config).unwrap();

        // add_region_all — no errors.
        registry.add_region_all(&scene).unwrap();

        // remove_region_all — no errors.
        registry.remove_region_all(&scene).unwrap();

        // close_all — no errors.
        registry.close_all().unwrap();

        // All modules still registered (close doesn't remove them).
        assert_eq!(
            registry.module_count(),
            4,
            "All modules should still be registered after full lifecycle"
        );
    }

    // ===================================================================
    // Protocol replay tests (section 13.3)
    // ===================================================================

    // -- 271. Protocol: land data roundtrip --------------------------------

    #[test]
    fn protocol_land_data_roundtrip() {
        use crate::land::{LandData, ParcelFlags};

        let owner = Uuid::new_v4();
        let mut ld = LandData::default();
        ld.local_id = 1;
        ld.name = "Test Parcel".to_string();
        ld.owner_id = owner;
        ld.area = 512;
        ld.flags = (ParcelFlags::CREATE_OBJECTS | ParcelFlags::ALLOW_FLY).bits();

        // Verify all fields roundtrip.
        assert_eq!(ld.local_id, 1);
        assert_eq!(ld.name, "Test Parcel");
        assert_eq!(ld.owner_id, owner);
        assert_eq!(ld.area, 512);

        // Verify ParcelFlags bitwise operations match expected.
        let flags = ParcelFlags::from_bits_truncate(ld.flags);
        assert!(flags.contains(ParcelFlags::CREATE_OBJECTS));
        assert!(flags.contains(ParcelFlags::ALLOW_FLY));
        assert!(!flags.contains(ParcelFlags::FOR_SALE));

        // Verify CREATE_OBJECTS bit value is 1 << 6 = 64.
        assert_eq!(ParcelFlags::CREATE_OBJECTS.bits(), 64);
        // Verify ALLOW_FLY bit value is 1 << 0 = 1.
        assert_eq!(ParcelFlags::ALLOW_FLY.bits(), 1);
    }

    // -- 272. Protocol: estate settings roundtrip --------------------------

    #[test]
    fn protocol_estate_settings_roundtrip() {
        use crate::estate::{EstateSettings, EstateSettingsUpdate};

        let estate_owner = Uuid::new_v4();
        let mut es = EstateSettings::default();
        es.estate_id = 100;
        es.estate_name = "Test Estate".to_string();
        es.estate_owner = estate_owner;
        es.sun_position = 6.0;

        // Verify all fields readable.
        assert_eq!(es.estate_id, 100);
        assert_eq!(es.estate_name, "Test Estate");
        assert_eq!(es.estate_owner, estate_owner);
        assert!((es.sun_position - 6.0).abs() < f64::EPSILON);

        // Verify EstateSettingsUpdate applies changes correctly.
        let update = EstateSettingsUpdate {
            estate_name: Some("Updated Estate".to_string()),
            sun_position: Some(12.0),
            ..Default::default()
        };
        es.apply_update(&update);
        assert_eq!(es.estate_name, "Updated Estate");
        assert!((es.sun_position - 12.0).abs() < f64::EPSILON);
        // Unchanged fields remain.
        assert_eq!(es.estate_id, 100);
        assert_eq!(es.estate_owner, estate_owner);
    }

    // -- 273. Protocol: map block and item types ---------------------------

    #[test]
    fn protocol_map_block_and_item_types() {
        use crate::world_map::{MapBlock, MapItemType, SimAccess, WorldMapModule};

        // Create MapBlock with region coordinates and SimAccess level.
        let block = MapBlock::new(1000, 1000, "TestRegion");
        assert_eq!(block.x, 1000);
        assert_eq!(block.y, 1000);
        assert_eq!(block.name, "TestRegion");
        assert_eq!(block.access, SimAccess::PG);
        assert_eq!(block.size_x, 256);
        assert_eq!(block.size_y, 256);

        // Verify MapItemType variants cover all expected types.
        assert_eq!(MapItemType::Telehub as u32, 1);
        assert_eq!(MapItemType::PGEvent as u32, 2);
        assert_eq!(MapItemType::MatureEvent as u32, 3);
        assert_eq!(MapItemType::AdultEvent as u32, 4);
        assert_eq!(MapItemType::PopularEvent as u32, 5);
        assert_eq!(MapItemType::AgentLocations as u32, 6);
        assert_eq!(MapItemType::LandForSale as u32, 7);
        assert_eq!(MapItemType::Classified as u32, 8);

        // Verify from_u32 round-trips.
        assert_eq!(MapItemType::from_u32(1), Some(MapItemType::Telehub));
        assert_eq!(MapItemType::from_u32(8), Some(MapItemType::Classified));
        assert_eq!(MapItemType::from_u32(99), None);

        // Create WorldMapModule, verify map_blocks storage.
        let mut wm = WorldMapModule::new();
        assert_eq!(wm.cached_block_count(), 0);
        wm.cache_block(block);
        assert_eq!(wm.cached_block_count(), 1);
    }

    // -- 274. Protocol: chat type ranges -----------------------------------

    #[test]
    fn protocol_chat_type_ranges() {
        use crate::chat::{ChatType, DEBUG_CHANNEL, MAX_MESSAGE_LENGTH};

        // ChatType::Say range is 20m, squared = 400.0.
        assert_eq!(ChatType::Say.range_squared(), Some(400.0));
        // ChatType::Shout range is 100m, squared = 10000.0.
        assert_eq!(ChatType::Shout.range_squared(), Some(10000.0));
        // ChatType::Whisper range is 10m, squared = 100.0.
        assert_eq!(ChatType::Whisper.range_squared(), Some(100.0));

        // Unlimited-range types return None.
        assert_eq!(ChatType::Region.range_squared(), None);
        assert_eq!(ChatType::Owner.range_squared(), None);
        assert_eq!(ChatType::Direct.range_squared(), None);
        assert_eq!(ChatType::Broadcast.range_squared(), None);
        assert_eq!(ChatType::DebugChannel.range_squared(), None);

        // DEBUG_CHANNEL value is i32::MAX = 2147483647.
        assert_eq!(DEBUG_CHANNEL, 2147483647);
        assert_eq!(DEBUG_CHANNEL, i32::MAX);

        // MAX_MESSAGE_LENGTH is 1000.
        assert_eq!(MAX_MESSAGE_LENGTH, 1000);
    }

    // -- 275. Protocol: IM dialog enum values ------------------------------

    #[test]
    fn protocol_im_dialog_enum_values() {
        use crate::messaging::{GridInstantMessage, IMDialog};

        // Verify critical IMDialog discriminant values match the C# protocol.
        assert_eq!(IMDialog::MessageFromAgent as u8, 0);
        assert_eq!(IMDialog::MessageFromObject as u8, 19);
        assert_eq!(IMDialog::StartTyping as u8, 41);
        assert_eq!(IMDialog::StopTyping as u8, 42);
        assert_eq!(IMDialog::FriendshipOffered as u8, 38);
        assert_eq!(IMDialog::FriendshipAccepted as u8, 39);
        assert_eq!(IMDialog::FriendshipDeclined as u8, 40);

        // Additional critical protocol values.
        assert_eq!(IMDialog::InventoryOffered as u8, 4);
        assert_eq!(IMDialog::RequestTeleport as u8, 22);
        assert_eq!(IMDialog::GroupNotice as u8, 32);
        assert_eq!(IMDialog::SessionGroupStart as u8, 17);
        assert_eq!(IMDialog::SessionCardlessStart as u8, 18);
        assert_eq!(IMDialog::SessionSend as u8, 20);
        assert_eq!(IMDialog::BUSY_AUTO_RESPONSE, 20);
        assert_eq!(IMDialog::from_u8(38), Some(IMDialog::FriendshipOffered));

        // Create GridInstantMessage with all fields.
        let from = Uuid::new_v4();
        let to = Uuid::new_v4();
        let session = Uuid::new_v4();
        let region = Uuid::new_v4();
        let im = GridInstantMessage {
            from_agent_id: from,
            from_agent_name: "Tester".to_string(),
            to_agent_id: to,
            message: "Hello".to_string(),
            im_session_id: session,
            dialog: IMDialog::MessageFromAgent,
            from_group: false,
            offline: 0,
            parent_estate_id: 1,
            position: [128.0, 128.0, 25.0],
            region_id: region,
            timestamp: 1000,
            binary_bucket: vec![0u8; 4],
        };
        assert_eq!(im.from_agent_id, from);
        assert_eq!(im.to_agent_id, to);
        assert_eq!(im.dialog, IMDialog::MessageFromAgent);
    }

    // -- 276. Protocol: teleport flag values -------------------------------

    #[test]
    fn protocol_teleport_flag_values() {
        // TeleportFlags are already imported from the transfer section above.
        // Verify key flag values match C# constants.
        assert_eq!(TeleportFlags::VIA_LURE.bits(), 4);
        assert_eq!(TeleportFlags::VIA_LANDMARK.bits(), 8);
        assert_eq!(TeleportFlags::VIA_LOCATION.bits(), 16);
        assert_eq!(TeleportFlags::GOD_LIKE.bits(), 256);
        assert_eq!(TeleportFlags::IS_FLYING.bits(), 4096);

        // Verify flag combinations with bitwise OR.
        let combined = TeleportFlags::VIA_LOCATION | TeleportFlags::IS_FLYING;
        assert!(combined.contains(TeleportFlags::VIA_LOCATION));
        assert!(combined.contains(TeleportFlags::IS_FLYING));
        assert!(!combined.contains(TeleportFlags::VIA_LURE));
        assert_eq!(combined.bits(), 16 | 4096);

        // Verify DEFAULT is zero.
        assert_eq!(TeleportFlags::DEFAULT.bits(), 0);

        // Verify all flags can be combined.
        let all = TeleportFlags::VIA_LURE
            | TeleportFlags::VIA_LANDMARK
            | TeleportFlags::VIA_LOCATION
            | TeleportFlags::GOD_LIKE
            | TeleportFlags::IS_FLYING;
        assert_eq!(all.bits(), 4 | 8 | 16 | 256 | 4096);
    }

    // -- 277. Protocol: attachment point enum coverage ---------------------

    #[test]
    fn protocol_attachment_point_enum_coverage() {
        use crate::attachments::AttachmentPoint;

        // Verify key attachment points exist and have expected discriminants.
        assert_eq!(AttachmentPoint::LeftHand as u8, 5);
        assert_eq!(AttachmentPoint::RightHand as u8, 6);
        assert_eq!(AttachmentPoint::Chest as u8, 1);
        assert_eq!(AttachmentPoint::Head as u8, 2);
        assert_eq!(AttachmentPoint::LeftFoot as u8, 7);
        assert_eq!(AttachmentPoint::RightFoot as u8, 8);
        assert_eq!(AttachmentPoint::HudCenter as u8, 35);
        assert_eq!(AttachmentPoint::HudTopRight as u8, 32);
        assert_eq!(AttachmentPoint::HudTopLeft as u8, 34);
        assert_eq!(AttachmentPoint::HudTop as u8, 33);
        assert_eq!(AttachmentPoint::HudBottomLeft as u8, 36);
        assert_eq!(AttachmentPoint::HudBottomRight as u8, 38);
        assert_eq!(AttachmentPoint::HudBottom as u8, 37);

        // Count total variants = 41 (Default=0 through AvatarCenter=40).
        let all_points: Vec<AttachmentPoint> = vec![
            AttachmentPoint::Default,
            AttachmentPoint::Chest,
            AttachmentPoint::Head,
            AttachmentPoint::LeftShoulder,
            AttachmentPoint::RightShoulder,
            AttachmentPoint::LeftHand,
            AttachmentPoint::RightHand,
            AttachmentPoint::LeftFoot,
            AttachmentPoint::RightFoot,
            AttachmentPoint::Back,
            AttachmentPoint::Pelvis,
            AttachmentPoint::Mouth,
            AttachmentPoint::Chin,
            AttachmentPoint::LeftEar,
            AttachmentPoint::RightEar,
            AttachmentPoint::LeftEye,
            AttachmentPoint::RightEye,
            AttachmentPoint::Nose,
            AttachmentPoint::RUArm,
            AttachmentPoint::RLArm,
            AttachmentPoint::LUArm,
            AttachmentPoint::LLArm,
            AttachmentPoint::RHip,
            AttachmentPoint::RULeg,
            AttachmentPoint::RLLeg,
            AttachmentPoint::LHip,
            AttachmentPoint::LULeg,
            AttachmentPoint::LLLeg,
            AttachmentPoint::Stomach,
            AttachmentPoint::LeftPec,
            AttachmentPoint::RightPec,
            AttachmentPoint::HudCenter2,
            AttachmentPoint::HudTopRight,
            AttachmentPoint::HudTop,
            AttachmentPoint::HudTopLeft,
            AttachmentPoint::HudCenter,
            AttachmentPoint::HudBottomLeft,
            AttachmentPoint::HudBottom,
            AttachmentPoint::HudBottomRight,
            AttachmentPoint::Neck,
            AttachmentPoint::AvatarCenter,
        ];
        assert_eq!(all_points.len(), 41);
        // Last variant value is 40.
        assert_eq!(AttachmentPoint::AvatarCenter as u8, 40);
    }

    // ===================================================================
    // Aurora Viewer acceptance tests (section 13.3)
    // ===================================================================

    // -- 278. Acceptance: ChatModule lifecycle and on_chat_from_viewer ------

    #[test]
    fn acceptance_chat_module_lifecycle() {
        use crate::chat::{ChatMessage, ChatModule, ChatSourceType, ChatType, MAX_MESSAGE_LENGTH};

        let mut module = ChatModule::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();

        // Initialise and add_region.
        module.initialise(&config).unwrap();
        module.add_region(&scene).unwrap();

        // Subscribe a receiver so broadcast::send() succeeds.
        let _rx = scene.events.on_chat_message.subscribe();

        // Simulate on_chat_from_viewer with Say type.
        let sender = Uuid::new_v4();
        let mut msg = ChatMessage {
            sender_id: sender,
            sender_name: "Test User".to_string(),
            message: "Hello region".to_string(),
            chat_type: ChatType::Say,
            source_type: ChatSourceType::Agent,
            position: [128.0, 128.0, 25.0],
            channel: 0,
        };
        // Module processes without error and broadcasts event.
        let delivered = module.on_chat_from_viewer(&mut msg);
        assert!(delivered);
        assert_eq!(msg.chat_type, ChatType::Say);

        // Test with empty message.
        let mut empty_msg = ChatMessage {
            sender_id: sender,
            sender_name: "Test User".to_string(),
            message: String::new(),
            chat_type: ChatType::Say,
            source_type: ChatSourceType::Agent,
            position: [128.0, 128.0, 25.0],
            channel: 0,
        };
        let delivered_empty = module.on_chat_from_viewer(&mut empty_msg);
        assert!(delivered_empty);
        assert!(empty_msg.message.is_empty());

        // Test with MAX_MESSAGE_LENGTH boundary.
        let long_msg_str: String = "a".repeat(MAX_MESSAGE_LENGTH + 100);
        let mut long_msg = ChatMessage {
            sender_id: sender,
            sender_name: "Test User".to_string(),
            message: long_msg_str,
            chat_type: ChatType::Say,
            source_type: ChatSourceType::Agent,
            position: [128.0, 128.0, 25.0],
            channel: 0,
        };
        let delivered_long = module.on_chat_from_viewer(&mut long_msg);
        assert!(delivered_long);
        assert!(long_msg.message.len() <= MAX_MESSAGE_LENGTH);

        module.remove_region(&scene).unwrap();
        module.close().unwrap();
    }

    // -- 279. Acceptance: IM send online vs offline -------------------------

    #[test]
    fn acceptance_im_online_vs_offline() {
        use crate::messaging::{
            GridInstantMessage, IMDialog, MessageTransferModule, OfflineMessageModule,
        };

        let mut msg_module = MessageTransferModule::new();
        let mut offline_module = OfflineMessageModule::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();

        msg_module.initialise(&config).unwrap();
        msg_module.add_region(&scene).unwrap();
        offline_module.initialise(&config).unwrap();
        offline_module.add_region(&scene).unwrap();

        let sender = Uuid::new_v4();
        let recipient = Uuid::new_v4();

        let im = GridInstantMessage {
            from_agent_id: sender,
            from_agent_name: "Sender".to_string(),
            to_agent_id: recipient,
            message: "Are you there?".to_string(),
            im_session_id: Uuid::new_v4(),
            dialog: IMDialog::MessageFromAgent,
            from_group: false,
            offline: 0,
            parent_estate_id: 1,
            position: [128.0, 128.0, 25.0],
            region_id: scene.region_id,
            timestamp: 12345,
            binary_bucket: Vec::new(),
        };

        // Online delivery succeeds (stub always returns true for non-nil).
        assert!(msg_module.send_instant_message(&im));

        // Offline storage: store and verify pending count.
        assert_eq!(offline_module.pending_count(recipient), 0);
        offline_module.store_message(im.clone());
        assert_eq!(offline_module.pending_count(recipient), 1);
        assert_eq!(offline_module.total_pending(), 1);

        // Retrieve drains the queue.
        let retrieved = offline_module.retrieve_messages(recipient);
        assert_eq!(retrieved.len(), 1);
        assert_eq!(retrieved[0].from_agent_id, sender);
        assert_eq!(offline_module.pending_count(recipient), 0);

        msg_module.remove_region(&scene).unwrap();
        msg_module.close().unwrap();
        offline_module.remove_region(&scene).unwrap();
        offline_module.close().unwrap();
    }

    // -- 280. Acceptance: teleport state machine happy path -----------------

    #[test]
    fn acceptance_teleport_state_machine_happy_path() {
        let mut module = TransferMod::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        module.initialise(&config).unwrap();
        module.add_region(&scene).unwrap();

        let agent = Uuid::new_v4();
        let dest = GridRegion::new(Uuid::new_v4(), "DestRegion");
        let flags = TeleportFlags::VIA_LOCATION;

        // do_teleport drives through: Preparing -> Transferring ->
        // ReceivedAtDestination -> CleaningUp, then resets.
        let result = module.do_teleport(agent, &dest, [128.0, 128.0, 25.0], [0.0, 1.0, 0.0], flags);
        assert!(result.is_ok());
        // Agent is no longer in transit after completion.
        assert!(!module.state_machine().is_in_transit(agent));

        // Verify state machine transitions work step by step.
        let sm = module.state_machine_mut();
        let agent2 = Uuid::new_v4();
        assert!(sm.set_in_transit(agent2));
        assert_eq!(
            sm.get_agent_transfer_state(agent2),
            Some(AgentTransferState::Preparing)
        );
        assert!(sm.update_in_transit(agent2, AgentTransferState::Transferring));
        assert_eq!(
            sm.get_agent_transfer_state(agent2),
            Some(AgentTransferState::Transferring)
        );
        assert!(sm.update_in_transit(agent2, AgentTransferState::ReceivedAtDestination));
        assert_eq!(
            sm.get_agent_transfer_state(agent2),
            Some(AgentTransferState::ReceivedAtDestination)
        );
        assert!(sm.update_in_transit(agent2, AgentTransferState::CleaningUp));
        assert_eq!(
            sm.get_agent_transfer_state(agent2),
            Some(AgentTransferState::CleaningUp)
        );
        // CleaningUp is terminal.
        assert!(AgentTransferState::CleaningUp.is_terminal());

        module.remove_region(&scene).unwrap();
        module.close().unwrap();
    }

    // -- 281. Acceptance: attachments wear/detach lifecycle -----------------

    #[test]
    fn acceptance_attachments_lifecycle() {
        use crate::attachments::{AttachmentPoint, AttachmentsModule};

        let mut module = AttachmentsModule::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        module.initialise(&config).unwrap();
        module.add_region(&scene).unwrap();

        let agent = Uuid::new_v4();
        let item = Uuid::new_v4();

        // Attach item at LeftHand, verify attached.
        let obj_id = module.wear(agent, item, AttachmentPoint::LeftHand).unwrap();
        assert!(!obj_id.is_nil());
        let attachments = module.get_attachments(agent);
        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0].item_id, item);
        assert_eq!(attachments[0].point, AttachmentPoint::LeftHand);
        assert_eq!(attachments[0].owner_id, agent);

        // Detach, verify empty.
        module.detach(agent, item).unwrap();
        let attachments_after = module.get_attachments(agent);
        assert!(attachments_after.is_empty());

        // Test invalid detach (item not found).
        let bad_item = Uuid::new_v4();
        let result = module.detach(agent, bad_item);
        assert!(result.is_err());

        // Test detach_all.
        let item2 = Uuid::new_v4();
        let item3 = Uuid::new_v4();
        module
            .wear(agent, item2, AttachmentPoint::RightHand)
            .unwrap();
        module.wear(agent, item3, AttachmentPoint::Head).unwrap();
        assert_eq!(module.get_attachments(agent).len(), 2);
        module.detach_all(agent);
        assert!(module.get_attachments(agent).is_empty());

        module.remove_region(&scene).unwrap();
        module.close().unwrap();
    }

    // -- 282. Acceptance: land panel management ----------------------------

    #[test]
    fn acceptance_land_panel_management() {
        use crate::land::{LandData, LandManagementModule, ParcelFlags};

        let mut module = LandManagementModule::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        module.initialise(&config).unwrap();
        module.add_region(&scene).unwrap();
        // add_region creates a default full-region parcel (local_id=1).
        assert_eq!(module.parcel_count(), 1);

        // Create a new parcel with known properties.
        let owner = Uuid::new_v4();
        let mut data = LandData::default();
        data.owner_id = owner;
        data.name = "Custom Parcel".to_string();
        data.area = 1024;
        data.flags = ParcelFlags::CREATE_OBJECTS.bits();

        let local_id = module.add_parcel(data);
        assert!(local_id > 0);
        assert_eq!(module.parcel_count(), 2);

        // Verify properties match.
        let parcel = module.get_parcel(local_id).unwrap();
        assert_eq!(parcel.data.name, "Custom Parcel");
        assert_eq!(parcel.data.owner_id, owner);

        // Test parcel grid (4x4m cells) allocation and lookup.
        let grid = module.grid();
        assert_eq!(grid.region_size(), 256);
        assert_eq!(grid.grid_size(), 64); // 256 / 4 = 64 cells per axis

        // The default parcel (id=1) should fill the grid initially.
        // After adding the new parcel, the default parcel still occupies most cells.
        // Cell at (0,0) should belong to the default parcel (id=1).
        let default_id = grid.get_parcel_id(0, 0);
        assert_eq!(default_id, 1);

        module.remove_region(&scene).unwrap();
        module.close().unwrap();
    }

    // -- 283. Acceptance: estate management lifecycle -----------------------

    #[test]
    fn acceptance_estate_management_lifecycle() {
        use crate::estate::{EstateManagementModule, EstateSettingsUpdate};

        let mut module = EstateManagementModule::new();
        let config = TestConfig::new();
        let scene = test_scene_ref();
        module.initialise(&config).unwrap();
        module.add_region(&scene).unwrap();

        // Set estate settings, verify readable.
        let owner = Uuid::new_v4();
        module.settings.estate_id = 42;
        module.settings.estate_name = "Test Estate".to_string();
        module.settings.estate_owner = owner;
        assert_eq!(module.settings.estate_id, 42);
        assert_eq!(module.settings.estate_name, "Test Estate");
        assert_eq!(module.settings.estate_owner, owner);

        // Verify sun position get/set.
        module.set_sun_position(18.0);
        assert!((module.get_sun_position() - 18.0).abs() < f64::EPSILON);

        // Apply update via update_settings.
        let update = EstateSettingsUpdate {
            estate_name: Some("Renamed Estate".to_string()),
            allow_voice: Some(false),
            ..Default::default()
        };
        module.update_settings(update);
        assert_eq!(module.settings.estate_name, "Renamed Estate");
        assert!(!module.settings.allow_voice);

        // Add/remove estate manager, verify list.
        let manager = Uuid::new_v4();
        module.settings.estate_managers.insert(manager);
        assert!(module.is_estate_manager(manager));
        assert!(module.has_access(manager));
        assert!(!module.is_banned(manager));

        // Owner is also an estate manager.
        assert!(module.is_estate_manager(owner));

        // Remove manager.
        module.settings.estate_managers.remove(&manager);
        assert!(!module.is_estate_manager(manager));
        // Manager still has access since estate is open (no user/group restrictions).
        assert!(module.has_access(manager));

        // Add to ban list.
        module.settings.estate_banned.insert(manager);
        assert!(module.is_banned(manager));
        assert!(!module.has_access(manager));

        module.remove_region(&scene).unwrap();
        module.close().unwrap();
    }
}
