mod eep {
    include!("../src/environment/eep.rs");
}

mod eep_store {
    include!("../src/environment/eep_store.rs");
}

#[test]
fn standalone_storage_api_compiles() {
    use eep_store::{InMemoryRegionEnvironmentStore, RegionEnvironmentStore};
    let store = InMemoryRegionEnvironmentStore::new();
    let _ = store.load("region");
    let _ = store.save("region", eep::ViewerEnvironment::default());
    let _ = store.delete("region");
}
