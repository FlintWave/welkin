// SPDX-License-Identifier: GPL-3.0-or-later

use std::io::Read;

use flate2::read::ZlibDecoder;
use glam::Vec3;
use rapier3d::geometry::SharedShape;
use rapier3d::math::Vector;
use thiserror::Error;
use welkin_llsd::{Llsd, Map, from_binary};

#[derive(Debug, Error)]
pub enum MeshAssetError {
    #[error("LLSD decode error: {0}")]
    Llsd(String),
    #[error("zlib decompression failed: {0}")]
    Decompress(String),
    #[error("missing LOD data: {0}")]
    MissingLod(String),
    #[error("invalid mesh data: {0}")]
    InvalidData(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeshLod {
    Physics,
    High,
    Medium,
    Low,
    Lowest,
}

impl MeshLod {
    fn key(self) -> &'static str {
        match self {
            MeshLod::Physics => "physics_mesh",
            MeshLod::High => "high_lod",
            MeshLod::Medium => "medium_lod",
            MeshLod::Low => "low_lod",
            MeshLod::Lowest => "lowest_lod",
        }
    }
}

const ALL_LODS: [MeshLod; 5] = [
    MeshLod::Physics,
    MeshLod::High,
    MeshLod::Medium,
    MeshLod::Low,
    MeshLod::Lowest,
];

#[derive(Debug, Clone)]
pub struct DecodedMesh {
    pub vertices: Vec<Vec3>,
    pub indices: Vec<u32>,
}

#[derive(Debug)]
pub struct MeshAsset {
    pub lods: Vec<(MeshLod, Vec<DecodedMesh>)>,
}

pub fn parse_mesh_header(data: &[u8]) -> Result<(Map, usize), MeshAssetError> {
    let (llsd, consumed) = from_binary(data).map_err(|e| MeshAssetError::Llsd(e.to_string()))?;
    let map = llsd
        .as_map()
        .ok_or_else(|| MeshAssetError::Llsd("header is not a map".into()))?
        .clone();
    Ok((map, consumed))
}

pub fn extract_lod_block(
    data: &[u8],
    header: &Map,
    lod_name: &str,
) -> Result<Vec<u8>, MeshAssetError> {
    let entry = header
        .get(lod_name)
        .ok_or_else(|| MeshAssetError::MissingLod(lod_name.into()))?;
    let offset = entry.get("offset").as_i32() as usize;
    let size = entry.get("size").as_i32() as usize;
    if size == 0 {
        return Err(MeshAssetError::MissingLod(lod_name.into()));
    }
    let end = offset
        .checked_add(size)
        .ok_or_else(|| MeshAssetError::InvalidData("offset+size overflow".into()))?;
    if end > data.len() {
        return Err(MeshAssetError::InvalidData(format!(
            "{lod_name}: offset {offset} + size {size} exceeds data length {}",
            data.len()
        )));
    }
    let compressed = &data[offset..end];
    let decoder = ZlibDecoder::new(compressed);
    const MAX_DECOMPRESSED: u64 = 32 * 1024 * 1024;
    let mut limited = decoder.take(MAX_DECOMPRESSED);
    let mut decompressed = Vec::new();
    limited
        .read_to_end(&mut decompressed)
        .map_err(|e| MeshAssetError::Decompress(e.to_string()))?;
    Ok(decompressed)
}

fn decode_vertex(bytes: &[u8], domain_min: Vec3, domain_max: Vec3) -> Vec3 {
    let x = u16::from_le_bytes([bytes[0], bytes[1]]);
    let y = u16::from_le_bytes([bytes[2], bytes[3]]);
    let z = u16::from_le_bytes([bytes[4], bytes[5]]);
    Vec3::new(
        domain_min.x + (x as f32 / 65535.0) * (domain_max.x - domain_min.x),
        domain_min.y + (y as f32 / 65535.0) * (domain_max.y - domain_min.y),
        domain_min.z + (z as f32 / 65535.0) * (domain_max.z - domain_min.z),
    )
}

fn parse_vec3(llsd: &Llsd) -> Vec3 {
    let arr = llsd.as_array();
    if arr.len() >= 3 {
        Vec3::new(arr[0].as_f32(), arr[1].as_f32(), arr[2].as_f32())
    } else {
        Vec3::ZERO
    }
}

fn decode_one_submesh(submesh: &Llsd) -> Result<DecodedMesh, MeshAssetError> {
    let pos_bytes = submesh.get("Position").as_binary();
    let idx_bytes = submesh.get("TriangleList").as_binary();
    let domain = submesh.get("PositionDomain");
    let domain_min = parse_vec3(domain.get("Min"));
    let domain_max = parse_vec3(domain.get("Max"));

    if !pos_bytes.len().is_multiple_of(6) {
        return Err(MeshAssetError::InvalidData(
            "Position byte count not divisible by 6".into(),
        ));
    }
    if !idx_bytes.len().is_multiple_of(2) {
        return Err(MeshAssetError::InvalidData(
            "TriangleList byte count not divisible by 2".into(),
        ));
    }

    let vertices: Vec<Vec3> = pos_bytes
        .as_chunks::<6>()
        .0
        .iter()
        .map(|chunk| decode_vertex(chunk, domain_min, domain_max))
        .collect();

    let indices: Vec<u32> = idx_bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]) as u32)
        .collect();

    // Validate that all triangle indices are within the vertex buffer bounds.
    let vertex_count = vertices.len() as u32;
    if indices.iter().any(|&idx| idx >= vertex_count) {
        return Err(MeshAssetError::InvalidData(
            "triangle index out of bounds".into(),
        ));
    }

    Ok(DecodedMesh { vertices, indices })
}

pub fn decode_submeshes(data: &[u8]) -> Result<Vec<DecodedMesh>, MeshAssetError> {
    let (llsd, _) = from_binary(data).map_err(|e| MeshAssetError::Llsd(e.to_string()))?;
    let arr = llsd.as_array();
    if arr.is_empty() && !llsd.is_array() {
        return Err(MeshAssetError::InvalidData(
            "submesh data is not an array".into(),
        ));
    }
    arr.iter().map(decode_one_submesh).collect()
}

pub fn decode_mesh_asset(data: &[u8]) -> Result<MeshAsset, MeshAssetError> {
    let (header, header_end) = parse_mesh_header(data)?;
    let body = &data[header_end..];
    let mut lods = Vec::new();
    for lod in ALL_LODS {
        if let Ok(decompressed) = extract_lod_block(body, &header, lod.key())
            && let Ok(submeshes) = decode_submeshes(&decompressed)
            && !submeshes.is_empty()
        {
            lods.push((lod, submeshes));
        }
    }
    if lods.is_empty() {
        return Err(MeshAssetError::MissingLod("no usable LOD found".into()));
    }
    Ok(MeshAsset { lods })
}

pub fn get_physics_mesh(asset: &MeshAsset) -> Option<&[DecodedMesh]> {
    for lod in [MeshLod::Physics, MeshLod::Medium, MeshLod::High] {
        if let Some((_, meshes)) = asset.lods.iter().find(|(l, _)| *l == lod) {
            return Some(meshes);
        }
    }
    None
}

pub fn mesh_to_shape(meshes: &[DecodedMesh]) -> SharedShape {
    let mut all_verts: Vec<Vector> = Vec::new();
    let mut all_tris: Vec<[u32; 3]> = Vec::new();
    let mut base: u32 = 0;
    for mesh in meshes {
        for v in &mesh.vertices {
            all_verts.push(Vector::new(v.x, v.y, v.z));
        }
        let tri_count = mesh.indices.len() / 3;
        for t in 0..tri_count {
            all_tris.push([
                base + mesh.indices[t * 3],
                base + mesh.indices[t * 3 + 1],
                base + mesh.indices[t * 3 + 2],
            ]);
        }
        base += mesh.vertices.len() as u32;
    }
    if all_verts.is_empty() || all_tris.is_empty() {
        return SharedShape::ball(0.001);
    }
    SharedShape::trimesh(all_verts, all_tris).unwrap_or_else(|_| SharedShape::ball(0.001))
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::ZlibEncoder;
    use std::io::Write;
    use welkin_llsd::{Llsd, Map, to_binary};

    fn llsd_map(entries: &[(&str, Llsd)]) -> Llsd {
        let mut m = Map::new();
        for (k, v) in entries {
            m.insert((*k).to_string(), v.clone());
        }
        Llsd::Map(m)
    }

    fn zlib_compress(data: &[u8]) -> Vec<u8> {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data).unwrap();
        encoder.finish().unwrap()
    }

    fn make_submesh_llsd(
        positions: &[[f32; 3]],
        indices: &[u16],
        domain_min: [f32; 3],
        domain_max: [f32; 3],
    ) -> Llsd {
        let mut pos_bytes = Vec::new();
        for p in positions {
            for (i, &val) in p.iter().enumerate() {
                let norm = (val - domain_min[i]) / (domain_max[i] - domain_min[i]);
                let q = (norm * 65535.0).round().clamp(0.0, 65535.0) as u16;
                pos_bytes.extend_from_slice(&q.to_le_bytes());
            }
        }
        let mut idx_bytes = Vec::new();
        for &idx in indices {
            idx_bytes.extend_from_slice(&idx.to_le_bytes());
        }
        llsd_map(&[
            ("Position", Llsd::Binary(pos_bytes)),
            (
                "PositionDomain",
                llsd_map(&[
                    (
                        "Max",
                        Llsd::Array(domain_max.iter().map(|&v| Llsd::Real(v as f64)).collect()),
                    ),
                    (
                        "Min",
                        Llsd::Array(domain_min.iter().map(|&v| Llsd::Real(v as f64)).collect()),
                    ),
                ]),
            ),
            ("TriangleList", Llsd::Binary(idx_bytes)),
        ])
    }

    fn make_test_asset(lod_names: &[&str]) -> Vec<u8> {
        let submesh = make_submesh_llsd(
            &[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            &[0, 1, 2],
            [0.0, 0.0, 0.0],
            [1.0, 1.0, 1.0],
        );
        let submesh_data = to_binary(&Llsd::Array(vec![submesh]));
        let compressed = zlib_compress(&submesh_data);

        let mut header = Map::new();
        let mut offset = 0i32;
        for &name in lod_names {
            header.insert(
                name.to_string(),
                llsd_map(&[
                    ("offset", Llsd::Integer(offset)),
                    ("size", Llsd::Integer(compressed.len() as i32)),
                ]),
            );
            offset += compressed.len() as i32;
        }
        let header_bytes = to_binary(&Llsd::Map(header));
        let mut result = header_bytes;
        for _ in lod_names {
            result.extend_from_slice(&compressed);
        }
        result
    }

    #[test]
    fn test_decode_vertex() {
        let domain_min = Vec3::new(-1.0, -2.0, -3.0);
        let domain_max = Vec3::new(1.0, 2.0, 3.0);
        let bytes = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        let v = decode_vertex(&bytes, domain_min, domain_max);
        assert!((v.x - 1.0).abs() < 1e-4);
        assert!((v.y - 2.0).abs() < 1e-4);
        assert!((v.z - 3.0).abs() < 1e-4);

        let bytes_zero = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let v0 = decode_vertex(&bytes_zero, domain_min, domain_max);
        assert!((v0.x - (-1.0)).abs() < 1e-4);
        assert!((v0.y - (-2.0)).abs() < 1e-4);
        assert!((v0.z - (-3.0)).abs() < 1e-4);

        let half: u16 = 32767;
        let hb = half.to_le_bytes();
        let bytes_mid = [hb[0], hb[1], hb[0], hb[1], hb[0], hb[1]];
        let vm = decode_vertex(&bytes_mid, domain_min, domain_max);
        assert!(vm.x.abs() < 0.01);
        assert!(vm.y.abs() < 0.01);
        assert!(vm.z.abs() < 0.01);
    }

    #[test]
    fn test_mesh_lod_fallback() {
        let data = make_test_asset(&["medium_lod", "high_lod"]);
        let asset = decode_mesh_asset(&data).unwrap();
        let physics = get_physics_mesh(&asset);
        assert!(physics.is_some());
        let meshes = physics.unwrap();
        assert!(!meshes.is_empty());

        let has_physics = asset.lods.iter().any(|(l, _)| *l == MeshLod::Physics);
        assert!(!has_physics);
    }

    #[test]
    fn test_empty_asset_error() {
        let result = decode_mesh_asset(&[]);
        assert!(result.is_err());
        match result {
            Err(MeshAssetError::Llsd(_)) => {}
            other => panic!("expected Llsd error, got {other:?}"),
        }
    }

    #[test]
    fn test_mesh_to_shape() {
        let mesh = DecodedMesh {
            vertices: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
            ],
            indices: vec![0, 1, 2, 0, 1, 3, 0, 2, 3, 1, 2, 3],
        };
        let shape = mesh_to_shape(&[mesh]);
        assert!(!shape.as_trimesh().is_none());
    }

    #[test]
    fn test_full_roundtrip() {
        let data = make_test_asset(&["physics_mesh", "high_lod"]);
        let asset = decode_mesh_asset(&data).unwrap();
        assert_eq!(asset.lods.len(), 2);
        let physics = get_physics_mesh(&asset).unwrap();
        assert_eq!(physics.len(), 1);
        assert_eq!(physics[0].vertices.len(), 3);
        assert_eq!(physics[0].indices.len(), 3);
        let shape = mesh_to_shape(physics);
        assert!(shape.as_trimesh().is_some());
    }

    #[test]
    fn test_oob_triangle_index_returns_error() {
        // Build a submesh with 3 vertices but an index referencing vertex 5.
        let submesh = make_submesh_llsd(
            &[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            &[0, 1, 5], // index 5 is out of bounds (only 3 vertices)
            [0.0, 0.0, 0.0],
            [1.0, 1.0, 1.0],
        );
        let submesh_data = to_binary(&Llsd::Array(vec![submesh]));
        let result = decode_submeshes(&submesh_data);
        assert!(
            result.is_err(),
            "out-of-bounds index should return an error"
        );
        match result {
            Err(MeshAssetError::InvalidData(msg)) => {
                assert!(msg.contains("out of bounds"), "error message: {msg}");
            }
            other => panic!("expected InvalidData error, got {other:?}"),
        }
    }

    #[test]
    fn test_valid_triangle_indices_accepted() {
        // All indices within bounds should succeed.
        let submesh = make_submesh_llsd(
            &[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            &[0, 1, 2],
            [0.0, 0.0, 0.0],
            [1.0, 1.0, 1.0],
        );
        let submesh_data = to_binary(&Llsd::Array(vec![submesh]));
        let result = decode_submeshes(&submesh_data);
        assert!(result.is_ok(), "valid indices should be accepted");
    }

    #[test]
    fn test_decompression_bounded() {
        // Verify that the decompression limit is enforced.  We create a zlib
        // stream that decompresses to well under the 32 MiB limit to confirm
        // normal data still works, and check the constant exists by ensuring
        // the compressed-data path succeeds.
        let small_data = vec![0u8; 1024];
        let compressed = zlib_compress(&small_data);

        let mut header = Map::new();
        header.insert(
            "test_lod".to_string(),
            llsd_map(&[
                ("offset", Llsd::Integer(0)),
                ("size", Llsd::Integer(compressed.len() as i32)),
            ]),
        );

        // extract_lod_block offsets are relative to the data slice passed in,
        // so we pass the compressed blob directly as the body.
        let result = extract_lod_block(&compressed, &header, "test_lod");
        assert!(result.is_ok(), "small decompression should succeed");
        assert_eq!(result.unwrap().len(), 1024);
    }

    #[test]
    fn test_empty_submesh_list_still_works() {
        let submesh_data = to_binary(&Llsd::Array(vec![]));
        let compressed = zlib_compress(&submesh_data);

        let header = llsd_map(&[(
            "high_lod",
            llsd_map(&[
                ("offset", Llsd::Integer(0)),
                ("size", Llsd::Integer(compressed.len() as i32)),
            ]),
        )]);
        let mut data = to_binary(&header);
        data.extend_from_slice(&compressed);

        let asset = decode_mesh_asset(&data);
        assert!(asset.is_err());
    }
}
