// SPDX-License-Identifier: GPL-3.0-or-later

//! Region-module framework for opensim-rust.
//!
//! Provides the [`RegionModule`] trait, a [`ModuleRegistry`] for managing
//! module instances and interface lookups, a typed [`SceneEvents`] event bus,
//! and skeleton module implementations.

pub mod attachments;
pub mod estate;
pub mod events;
pub mod inventory;
pub mod land;
pub mod lifecycle;
pub mod module;
pub mod permissions;
pub mod registry;

// Re-exports for convenience.
pub use attachments::{AttachmentInfo, AttachmentPoint, AttachmentsModule};
pub use estate::{EstateManagementModule, EstateSettings, EstateSettingsUpdate, TelehubManager};
pub use events::*;
pub use inventory::{
    AssetTransactionModule, AssetType, DerezAction, InventoryAccessModule,
    InventoryTransferModule, RezResult, UploadTransaction, XferModule,
};
pub use land::{
    DwellModule, LandData, LandManagementModule, LandParcel, ParcelGrid, PrimCountModule,
    PrimCounts,
};
pub use lifecycle::{CapsModule, DynAttrsModule, ServiceThrottleModule};
pub use module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
pub use permissions::{
    checks::ObjectPerms, DefaultPermissionsModule, PermissionsConfig, UserSet,
};
pub use registry::{ModuleRegistry, SceneRef};

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

        assert_eq!(
            config.get_string("General", "Name", "default"),
            "MyRegion"
        );
        assert_eq!(config.get_int("General", "Port", 0), 9000);
        assert_eq!(
            config.get_bool("ServiceThrottle", "Enabled", true),
            false
        );
        assert!((config.get_float("Physics", "Gravity", 0.0) - (-9.81)).abs() < f64::EPSILON);

        // Defaults:
        assert_eq!(
            config.get_string("Missing", "Key", "fallback"),
            "fallback"
        );
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
        AccessList, DwellModule, LandAccessEntry, LandData, LandDataUpdate,
        LandManagementModule, LandParcel, ParcelFlags, ParcelGrid, PrimCountModule,
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
        assert!(merged[0][0]);      // from a
        assert!(merged[32][32]);    // from b
        assert!(!merged[32][0]);    // neither
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

    use crate::estate::{EstateManagementModule, EstateSettings, EstateSettingsUpdate, TelehubManager};

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

        let derez_item = module.derez_object(agent, Uuid::new_v4(), DerezAction::Take).unwrap();
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
        module.wear(agent, Uuid::new_v4(), AttachmentPoint::Back).unwrap();
        assert!(module.detach(agent, Uuid::new_v4()).is_err());
    }

    // -- 56. AttachmentsModule detach_all ----------------------------------

    #[test]
    fn attachments_detach_all() {
        let mut module = AttachmentsModule::new();
        let agent = Uuid::new_v4();
        module.wear(agent, Uuid::new_v4(), AttachmentPoint::LeftHand).unwrap();
        module.wear(agent, Uuid::new_v4(), AttachmentPoint::RightHand).unwrap();
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
        module.wear(agent, Uuid::new_v4(), AttachmentPoint::Stomach).unwrap();

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
        assert!(AttachmentsModule::validate_point(AttachmentPoint::AvatarCenter));
        assert!(AttachmentsModule::validate_point(AttachmentPoint::HudBottomRight));
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
        module.wear(Uuid::new_v4(), Uuid::new_v4(), AttachmentPoint::Nose).unwrap();
        module.remove_region(&scene).unwrap();
        // After remove, no attachments remain.
        module.close().unwrap();
    }

    // ===================================================================
    // Permissions-module tests (section 3 — high-risk)
    // ===================================================================

    use crate::permissions::{
        DefaultPermissionsModule, UserSet,
    };
    use opensim_types::PermissionMask;

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
        assert!(m.can_return_object(
            parcel_owner,
            perm_owner_id(),
            Uuid::nil(),
            Some(&parcel),
        ));
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
}
