# Modern Avatar Features Tasks

## Phase 1: PBR materials

- [ ] Define PBR material struct (base color, metallic, roughness, normal, emissive, alpha mode, double-sided)
- [ ] Store PBR material LLSD as asset type alongside mesh data
- [ ] Implement RenderMaterials cap: GET returns all materials for objects in view, PUT stores new materials
- [ ] Wire GetMesh2 to include PBR material references in mesh asset response
- [ ] Verify PBR materials display in Aurora viewer (metallic sphere test, roughness gradient test)
- [ ] Integration test: upload mesh with PBR material, rez, verify material data round-trips

## Phase 2: Bakes on Mesh

- [ ] Wire bake cache: UploadBakedTexture stores baked layers keyed by bake type (head, upper, lower, eyes, skirt, hair)
- [ ] Implement bake compositing: combine wearable layers (skin, tattoo, undershirt, etc.) into final baked texture
- [ ] Wire mesh attachment bake references: mesh bodies reference bake texture UUIDs instead of explicit texture UUIDs
- [ ] Handle AppearanceData ObjectUpdate flags for BOM-enabled avatars (texture entry references bake channels)
- [ ] Verify avatar appearance: system clothing on mesh body displays correctly in Aurora
- [ ] Integration test: wear mesh body + system shirt, verify bake composites and renders on mesh

## Phase 3: Animesh

- [ ] Add AnimMeshData extra param to prim serialization (ExtraParams byte array)
- [ ] Parse Animesh skeleton data from mesh asset (joint positions, bind matrices)
- [ ] Wire animation delivery: Animesh prims carry animation asset UUIDs in object properties
- [ ] Handle ObjectAnimation message: server forwards animation state updates to viewers in range
- [ ] Verify Animesh object plays animation in Aurora viewer
- [ ] Integration test: rez Animesh prim, set animation via script, verify it animates for nearby viewers

## Phase 4: glTF import

- [ ] Add `gltf` 1.4.1 crate to workspace Cargo.toml
- [ ] Implement glTF parser: extract vertices, normals, UVs, indices, and PBR material data from glTF/glb
- [ ] Convert glTF mesh data to Welkin's internal mesh format (SL mesh asset format)
- [ ] Wire mesh upload cap to accept glTF/glb content type alongside existing mesh format
- [ ] Store converted mesh as standard mesh asset with PBR material references
- [ ] Verify round-trip: upload glTF in viewer, rez, see correct geometry and materials
- [ ] Integration test: upload a glTF model with multiple materials, verify all submeshes and materials display
