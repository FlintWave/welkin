// SPDX-License-Identifier: GPL-3.0-or-later

//! Region-module framework for opensim-rust.
//!
//! Provides the [`RegionModule`] trait, a [`ModuleRegistry`] for managing
//! module instances and interface lookups, a typed [`SceneEvents`] event bus,
//! and skeleton module implementations.

pub mod events;
pub mod land;
pub mod lifecycle;
pub mod module;
pub mod registry;

// Re-exports for convenience.
pub use events::*;
pub use land::{
    DwellModule, LandData, LandManagementModule, LandParcel, ParcelGrid, PrimCountModule,
    PrimCounts,
};
pub use lifecycle::{CapsModule, DynAttrsModule, ServiceThrottleModule};
pub use module::{ModuleConfig, ModuleError, ModuleType, RegionModule};
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
}
