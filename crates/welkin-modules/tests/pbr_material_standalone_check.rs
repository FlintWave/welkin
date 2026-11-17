#[path = "../src/avatar/pbr_material.rs"]
mod pbr_material;

#[test]
fn standalone_module_compiles_and_links() {
    use pbr_material::{PbrAlphaMode, PbrMaterial};
    let material = PbrMaterial::default();
    assert_eq!(material.alpha_mode, PbrAlphaMode::Opaque);
    let _ = material.roughness_clamped();
    let _ = material.to_llsd();
}
