# Modern Avatar Features Tasks

## Phase 1: PBR materials

- [x] Define PBR material struct (base color, metallic, roughness, normal, emissive, alpha mode, double-sided)
- [x] Store PBR material LLSD as asset type alongside mesh data
- [x] Implement RenderMaterials cap: GET returns all materials for objects in view, PUT stores new materials
- [x] Wire the GetMesh2 handler in crates/welkin-caps/src/handlers.rs to include associated PBR material references in its existing mesh response, without rewriting crates/welkin-modules/src/avatar/pbr_material.rs
- [x] Add unit evidence for PBR material serialization in crates/welkin-modules/src/avatar/pbr_material.rs and verify material payload construction through welkin-modules tests
- [x] Integration test: upload mesh with PBR material, rez, verify material data round-trips

## Phase 2: Bakes on Mesh

- [x] Wire bake cache: UploadBakedTexture stores baked layers keyed by bake type (head, upper, lower, eyes, skirt, hair)
- [x] Implement bake compositing: combine wearable layers (skin, tattoo, undershirt, etc.) into final baked texture
- [ ] Wire mesh attachment bake references in crates/welkin-region/src/main.rs only; reuse the existing bake texture byte array and do not create renderer crates or modules; keep cargo check -p welkin-region green
- [x] Handle AppearanceData ObjectUpdate flags for BOM-enabled avatars (texture entry references bake channels)
- [x] Verify avatar appearance: system clothing on mesh body displays correctly in Aurora
- [x] Integration test: wear mesh body + system shirt, verify bake composites and renders on mesh

## Phase 3: Animesh

- [ ] Add AnimMeshData extra param to prim serialization (ExtraParams byte array)
- [x] Parse Animesh skeleton data from mesh asset (joint positions, bind matrices)
- [ ] Wire animation delivery: Animesh prims carry animation asset UUIDs in object properties
- [x] Handle ObjectAnimation message: server forwards animation state updates to viewers in range
- [x] Verify Animesh object plays animation in Aurora viewer
- [x] Integration test: rez Animesh prim, set animation via script, verify it animates for nearby viewers

## Phase 4: glTF import

- [ ] Add `gltf` 1.4.1 crate to workspace Cargo.toml
- [ ] Implement glTF parser: extract vertices, normals, UVs, indices, and PBR material data from glTF/glb
- [x] Convert glTF mesh data to Welkin's internal mesh format (SL mesh asset format)
- [x] Wire mesh upload cap to accept glTF/glb content type alongside existing mesh format
- [x] Store converted mesh as standard mesh asset with PBR material references
- [x] Verify round-trip: upload glTF in viewer, rez, see correct geometry and materials
- [x] Integration test: upload a glTF model with multiple materials, verify all submeshes and materials display
