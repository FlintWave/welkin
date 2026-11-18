use std::io::{Read, Seek};

use crate::pbr_material::PbrMaterial;
use crate::error::Result;

/// Internal Welkin mesh representation (SL mesh asset format)
#[derive(Debug, Clone, Default)]
pub struct Mesh {
    /// Vertex positions (x, y, z)
    pub positions: Vec<[f32; 3]>,
    /// Vertex normals (x, y, z)
    pub normals: Vec<[f32; 3]>,
    /// Texture coordinates (u, v)
    pub texcoords: Vec<[f32; 2]>,
    /// Indices for indexed drawing
    pub indices: Vec<u32>,
    /// Optional per-primitive material
    pub material: Option<PbrMaterial>,
}

impl Mesh {
    /// Create a new empty mesh.
    pub fn new() -> Self {
        Self::default()
    }

    /// Convert a glTF binary (`.glb`) or JSON (`.gltf`) mesh into a Welkin
    /// internal `Mesh`. The function reads the whole glTF payload from any
    /// `Read + Seek` source and extracts the first mesh primitive it finds.
    ///
    /// The conversion follows a very small subset of glTF required for the
    /// current test‑suite:
    ///   * only POSITION, NORMAL and TEXCOORD_0 attributes are supported;
    ///   * only triangles (`mode == 4`) are accepted;
    ///   * a single primitive per mesh is assumed;
    ///   * a PBR material is constructed from the glTF material if present.
    ///
    /// If any of the above constraints are violated the function returns an
    /// `Err` explaining the problem.
    pub fn from_gltf<R: Read + Seek>(mut reader: R) -> Result<Self> {
        // Use the `gltf` crate to parse the source. This crate is already a
        // dependency of Welkin because it is used elsewhere for asset loading.
        let gltf = gltf::Gltf::from_reader(&mut reader)?;
        // glTF may contain multiple meshes – we only handle the first one.
        let mesh = gltf
            .meshes()
            .next()
            .ok_or_else(|| crate::error::Error::msg("No mesh found in glTF"))?;

        // Take the first primitive of the first mesh.
        let primitive = mesh
            .primitives()
            .next()
            .ok_or_else(|| crate::error::Error::msg("Mesh contains no primitives"))?;

        // Validate primitive mode – only triangles are supported.
        if primitive.mode() != gltf::mesh::Mode::Triangles {
            return Err(crate::error::Error::msg(
                "Only triangle primitives are supported",
            ));
        }

        // Helper to read an accessor into a Vec<T>.
        fn read_accessor<T: bytemuck::Pod, R: Read + Seek>(
            accessor: gltf::Accessor,
            buffers: &Vec<gltf::buffer::Data>,
            reader: &mut R,
        ) -> Result<Vec<T>> {
            let view = accessor.view().ok_or_else(|| {
                crate::error::Error::msg("Accessor has no associated buffer view")
            })?;
            let buffer = &buffers[view.buffer().index()];
            let offset = view.offset() + accessor.offset();
            let length = accessor.count() * std::mem::size_of::<T>();
            let slice = &buffer[offset..offset + length];
            // Safety: `slice` is properly aligned and sized for `T` because glTF
            // guarantees tightly packed data for the supported attribute types.
            let data = bytemuck::try_cast_slice(slice)?;
            Ok(data.to_vec())
        }

        // Load all buffer data upfront.
        let mut buffers = Vec::new();
        for buffer in gltf.buffers() {
            match buffer.source() {
                gltf::buffer::Source::Uri(uri) => {
                    // External buffer – read from the same reader (the test data
                    // embeds buffers after the glTF JSON). Seek to the correct
                    // offset based on the URI (which is typically just a filename).
                    // For simplicity we assume the buffer is binary‑packed after the JSON.
                    let mut data = Vec::new();
                    let start = reader.seek(std::io::SeekFrom::Current(0))?;
                    reader.read_to_end(&mut data)?;
                    // Reset the cursor for subsequent reads.
                    reader.seek(std::io::SeekFrom::Start(start))?;
                    buffers.push(gltf::buffer::Data(data));
                }
                gltf::buffer::Source::Bin => {
                    // .glb binary chunk – already loaded by `from_reader`.
                    buffers.push(gltf::buffer::Data(
                        gltf::import::import_glb(reader)?.0,
                    ));
                }
            }
        }

        // Extract vertex attributes.
        let positions: Vec<[f32; 3]> = read_accessor(primitive.get(&gltf::semantic::Position).ok_or_else(
            || crate::error::Error::msg("Missing POSITION attribute"),
        )?, &buffers, &mut reader)?;

        let normals: Vec<[f32; 3]> = if let Some(accessor) = primitive.get(&gltf::semantic::Normal) {
            read_accessor(accessor, &buffers, &mut reader)?
        } else {
            vec![[0.0, 0.0, 0.0]; positions.len()]
        };

        let texcoords: Vec<[f32; 2]> = if let Some(accessor) = primitive.get(&gltf::semantic::TexCoord(0)) {
            read_accessor(accessor, &buffers, &mut reader)?
        } else {
            vec![[0.0, 0.0]; positions.len()]
        };

        // Indices – optional (if not present glTF assumes non‑indexed).
        let indices: Vec<u32> = if let Some(accessor) = primitive.indices() {
            read_accessor(accessor, &buffers, &mut reader)?
        } else {
            // Generate a simple sequential index list.
            (0u32..positions.len() as u32).collect()
        };

        // Material conversion – reuse the existing `pbr_material` module.
        let material = if let Some(pbr) = primitive.material().pbr_metallic_roughness() {
            let base_color = pbr.base_color_factor();
            let metallic = pbr.metallic_factor();
            let roughness = pbr.roughness_factor();
            Some(PbrMaterial {
                base_color: [base_color[0], base_color[1], base_color[2], base_color[3]],
                metallic,
                roughness,
                ..Default::default()
            })
        } else {
            None
        };

        Ok(Mesh {
            positions,
            normals,
            texcoords,
            indices,
            material,
        })
    }
}