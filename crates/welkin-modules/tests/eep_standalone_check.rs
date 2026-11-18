#[path = "../src/environment/eep.rs"]
mod eep;

#[test]
fn standalone_defaults() {
    let environment = eep::ViewerEnvironment::default_region();
    assert!(environment.sky.name.contains("Default"));
    let _ = environment.day_cycle;
}
