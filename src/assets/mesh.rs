// Store PBR material LLSD alongside mesh data.
// Minimal change: add an optional PBR material payload to the mesh asset record.
pub struct MeshAsset {
    pub mesh_data: Vec<u8>,
    pub pbr_material_llsd: Option<Vec<u8>>,
}