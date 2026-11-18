// SPDX-License-Identifier: LGPL-3.0-or-later

use thiserror::Error;

#[derive(Debug, Error)]
pub enum GltfError {
    #[error("import failed: {0}")]
    Import(String),
    #[error("mesh contains no primitives")]
    NoPrimitive,
    #[error("missing position attribute")]
    MissingPositions,
}

#[derive(Debug, Clone, Default)]
pub struct GltfMesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub texcoords: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}

pub fn parse_gltf(bytes: &[u8]) -> Result<GltfMesh, GltfError> {
    let (document, buffers, _images) = gltf::import_slice(bytes)
        .map_err(|e| GltfError::Import(e.to_string()))?;
    let mesh: gltf::Mesh = document.meshes().next().ok_or(GltfError::NoPrimitive)?;
    let primitive = mesh.primitives().next().ok_or(GltfError::NoPrimitive)?;

    let buffer_get = |b: gltf::Buffer| -> Option<&[u8]> { buffers.get(b.index()).map(|d| d.0.as_slice()) };

    let reader = primitive.reader(buffer_get);
    let positions: Vec<[f32; 3]> = reader
        .read_positions()
        .map(|it| it.map(|p| [p[0], p[1], p[2]]).collect())
        .ok_or(GltfError::MissingPositions)?;

    // Normals
    let reader = primitive.reader(buffer_get);
    let normals = reader
        .read_normals()
        .map(|it| it.map(|n| [n[0], n[1], n[2]]).collect())
        .unwrap_or_else(|| vec![[0.0, 0.0, 1.0]; positions.len()]);

    // Texcoords
    let reader = primitive.reader(buffer_get);
    let texcoords = match reader.read_tex_coords(0) {
        Some(gltf::mesh::util::ReadTexCoords::F32(it)) => it.map(|t| [t[0], t[1]]).collect(),
        _ => vec![[0.0, 0.0]; positions.len()],
    };

    // Indices
    let mut indices = Vec::new();
    if primitive.indices().is_some() {
        let reader = primitive.reader(buffer_get);
        if let Some(read) = reader.read_indices() {
            match read {
                gltf::mesh::util::ReadIndices::U8(it) => {
                    indices.extend(it.map(|v| v as u32));
                }
                gltf::mesh::util::ReadIndices::U16(it) => {
                    indices.extend(it.map(|v| v as u32));
                }
                gltf::mesh::util::ReadIndices::U32(it) => {
                    indices.extend(it);
                }
            }
        }
    }
    if indices.is_empty() {
        indices = (0..positions.len() as u32).collect();
    }

    Ok(GltfMesh {
        positions,
        normals,
        texcoords,
        indices,
    })
}
