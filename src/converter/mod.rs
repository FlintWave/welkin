use crate::asset::mesh::MeshAsset;
use crate::pbr_material::PbrMaterial;
use crate::asset::AssetId;
use crate::asset::AssetStore;

pub fn store_converted_mesh(
    store: &mut AssetStore,
    vertices: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
    material: PbrMaterial,
) -> AssetId<MeshAsset> {
    // Store the PBR material first
    let material_id = store.store_asset(material);
    // Then store the mesh referencing the material
    let mesh = MeshAsset {
        vertices,
        normals,
        uvs,
        indices,
        material: material_id,
    };
    store.store_asset(mesh)
}