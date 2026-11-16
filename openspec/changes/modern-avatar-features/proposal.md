# Modern Avatar Features

## Motivation

Current viewers (Aurora, Firestorm) support PBR materials, Bakes on Mesh, Animesh, and glTF mesh import. OpenSim 0.9.3.0 has partial support for some of these; Welkin should match or exceed that baseline. These features are visible the moment an avatar appears and affect first impressions of a grid.

## Scope

### In scope

1. **PBR materials (GLTF materials)**: server-side storage and delivery of PBR material data (metallic, roughness, normal, emissive maps). The viewer handles rendering; the server stores material assets and delivers them via GetMesh2/ViewerAsset caps.
2. **Bakes on Mesh (BOM)**: server-side bake compositing. Avatar appearance layers (skin, tattoo, clothing) baked into a single texture and applied to mesh attachments. The UploadBakedTexture cap already exists; wire the bake cache and compositing pipeline so mesh bodies display wearable layers.
3. **Animesh**: server-side support for animated mesh objects. Animesh prims carry skeleton and animation data; the server stores this data and delivers it to viewers via the existing mesh caps. Requires the AnimMeshData extra param on prims.
4. **glTF mesh import**: accept glTF/glb uploads via the mesh upload cap, convert to internal mesh format, store as assets. The `gltf` crate (MIT/Apache-2.0) handles parsing.

### Out of scope

- Viewer-side rendering (that's Aurora/Firestorm's job)
- Custom shaders or post-processing
- PBR scene lighting (server doesn't control this)
- Full Animesh scripting API (llSetAnimOverride for Animesh is LSL work, tracked in lsl-compiler)

## Library search results

**Add to workspace:**
- `gltf` 1.4.1 (MIT/Apache-2.0) - glTF 2.0 parser, handles .gltf and .glb, 1.5M downloads on crates.io. Needed for glTF import.

**Already in workspace:**
- `reqwest` (MIT/Apache-2.0) - HTTP client for ROBUST service calls
- `image` (MIT) - texture processing, already used for bake textures
- `uuid` (MIT/Apache-2.0) - asset IDs

**Evaluated and deferred:**
- `jp2k` / `jpeg2000` - JPEG2000 texture codec. OpenSim stores textures as J2K; Welkin already handles this through the existing texture pipeline. No new crate needed unless we find a performance gap.
- `avatar-anim` - no stable crate exists for SL/OpenSim animation format. Parse BVH/anim data manually using the format spec from OpenSim's BVHReader.cs.

## Phases

### Phase 1: PBR materials (3 days)
Store PBR material LLSD alongside mesh assets. Return material data via GetMesh2 and the RenderMaterials cap. Verify materials display in Aurora.

### Phase 2: Bakes on Mesh (4 days, can parallel with Phase 1)
Wire the bake compositing pipeline: viewer uploads baked textures, server caches them, mesh attachments reference bake UUIDs. Verify avatar appearance renders correctly on mesh bodies.

### Phase 3: Animesh (3 days, depends on Phase 1)
Add AnimMeshData extra param to prim serialization. Deliver skeleton and animation data through mesh caps. Verify Animesh objects animate in viewer.

### Phase 4: glTF import (3 days, can parallel with Phase 3)
Add `gltf` crate. Accept glTF/glb uploads via mesh upload cap. Convert to internal format (extract vertices, normals, UVs, materials). Store as mesh assets. Verify round-trip: upload glTF in viewer, rez it, see correct geometry.

## Acceptance criteria

- PBR materials on prims render with metallic/roughness in Aurora
- Avatar wearing system clothing on a mesh body displays correctly (BOM)
- An Animesh object placed in-world plays its animation
- A glTF model uploaded through the viewer rezzes with correct geometry and materials
- All existing tests pass; no regressions in standard mesh or texture delivery

## Estimates

~13 working days total, ~9 day critical path (Phase 1 and 2 parallel, then Phase 3, then Phase 4 can overlap Phase 3).
