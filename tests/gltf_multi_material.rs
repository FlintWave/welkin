use welkin::server::Server;
use welkin::scene::{Scene, Mesh};
use welkin::gltf::GltfLoader;
use std::path::PathBuf;

#[tokio::test]
async fn upload_gltf_with_multiple_materials() {
    // Initialise a minimal server and scene (the exact APIs depend on Welkin's current design)
    let server = Server::new().await.expect("Failed to start server");
    let scene = Scene::new();

    // Path to a test glTF file containing multiple materials.
    // The repository should include this asset under `tests/assets/multi_material.gltf`.
    let gltf_path = PathBuf::from("tests/assets/multi_material.gltf");
    assert!(gltf_path.exists(), "Test glTF file not found");

    // Load the glTF file using Welkin's existing loader.
    let loader = GltfLoader::new(&gltf_path);
    let result = loader.load(&scene).await;
    assert!(result.is_ok(), "Failed to load glTF: {}", result.unwrap_err());

    // Verify that the scene now contains at least one mesh with multiple submeshes.
    let meshes: Vec<&Mesh> = scene.meshes().collect();
    assert!(!meshes.is_empty(), "No meshes were created from the glTF");

    // Find a mesh that has more than one submesh (i.e., multiple primitives/materials).
    let multi_submesh = meshes.iter().find(|m| m.submeshes().len() > 1);
    assert!(
        multi_submesh.is_some(),
        "Expected at least one mesh with multiple submeshes"
    );

    let mesh = multi_submesh.unwrap();
    let submesh_count = mesh.submeshes().len();
    let material_count = mesh.materials().len();

    // The test glTF is expected to have at least two materials and matching submeshes.
    assert!(
        submesh_count >= 2,
        "Expected at least two submeshes, found {}",
        submesh_count
    );
    assert!(
        material_count >= 2,
        "Expected at least two materials, found {}",
        material_count
    );

    // Ensure each submesh references a material.
    for submesh in mesh.submeshes() {
        let mat = submesh.material();
        assert!(
            material_count > mat.id(),
            "Submesh references a non‑existent material"
        );
    }

    // Clean up the server.
    server.shutdown().await;
}