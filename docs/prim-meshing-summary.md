# Prim meshing summary

Change `prim-meshing` (Phase 3): 15 of 15 tasks accepted. It turns prim
parameters, sculpt maps and mesh assets into Rapier colliders. Commits
`759e3c6` through `129d401`; main code in `crates/opensim-prim/src/meshing.rs`
(1,261 lines) and `crates/opensim-prim/src/mesh_asset.rs` (464 lines).

## What was built

| Area | Description |
|---|---|
| Analytic shapes | `is_box` / `is_sphere` detect plain prims; `shape_for_box` and `shape_for_sphere` return cuboid and ball colliders with no mesh |
| Volume meshing | `shape_from_volume` runs `VolumeParams` through `aurora-prim` and builds a trimesh `SharedShape` |
| LOD selection | `PhysicsLod::from_size` picks Lowest/Low/Medium/High by largest dimension (under 4, 16, 64 m, else High); `detail()` maps to 0.25, 0.5, 1.0, 2.0 |
| Shape cache | `ShapeCache`: thread-safe map keyed by a caller-supplied `u64` hash of params, size and shape type |
| Sculpt maps | `SculptType::from_raw` decodes type plus invert/mirror flags; `shape_from_sculpt` meshes a displacement grid supplied through the `SculptTextureSource` trait |
| Mesh asset decoder | `mesh_asset.rs`: LLSD header parse, per-LOD zlib block extraction, submesh decode, `get_physics_mesh` (falls back to visual LOD), `mesh_to_shape` |
| Convex decomposition | `decompose_convex` runs parry3d V-HACD with `ConvexDecompParams` (32 hulls, 100,000 voxel budget mapped to grid 16 to 256, concavity 0.0025); results cached in `DecompCache` |
| Linkset colliders | `create_linkset_shape` builds one compound collider from per-part shapes with relative transforms |
| Shape type switching | `PhysicsShapeType` (Prim, Convex, None, Mesh); `create_physics_shape` and `create_physics_shape_with_sculpt` are the entry points used by `add_prim` in `opensim-physics-rapier`, replacing the Phase 1 placeholders |

## Performance

`collider_proof.rs` meshes 1,000 mixed prims with unique cache keys (worst
case, no cache hits) against a 5 s budget. Measured throughput is about
54,000 prims/sec, far inside the budget. V-HACD is the expensive path, which
is why `DecompCache` exists.

## Tests

- `opensim-prim`: 46 tests (29 in `meshing.rs`, 9 in `mesh_asset.rs`, 8 in
  `tests/collider_proof.rs`).
- `opensim-physics-rapier`: 4 in `tests/add_prim_with_shape.rs`, 4 in
  `tests/shape_switching.rs`, 3 in the spike test.
- Proof tests cover reference prims (box, cylinder, torus, sculpt, mesh),
  drop tests on terrain and other objects, and the performance budget.

## Security review (task 6.4)

Mesh assets are untrusted user uploads, so the decoder had a second Claude
review. Result: conditional pass, with fixes in `1812e45`, each with a test.

| ID | Severity | Finding | Fix |
|---|---|---|---|
| CRITICAL-1 | Critical | Triangle indices were not checked against the vertex count in `decode_one_submesh`, so a crafted asset could panic parry3d BVH construction (out-of-bounds) | Indices validated; invalid assets return an error |
| HIGH-1 | High | Unbounded zlib decompression allowed a decompression bomb | Output capped at 32 MiB via `Read::take` |
| MEDIUM-3 | Medium | `.lock().unwrap()` on `ShapeCache` and `DecompCache` would cascade panics after a poisoned mutex | Locks recover with `unwrap_or_else(\|e\| e.into_inner())` |

## Known gaps

- JPEG 2000 sculpt decoding is not wired in. `SculptTextureSource` is a
  trait and the only implementation is `PlaceholderSculptSource` (returns
  `None`). The `aurora-assets` versus pure Rust decision is still open.
- `max_vertices_per_hull` is advisory: the parry3d V-HACD backend does not
  enforce it.
- V-HACD output is tuned for similar quality, not identical hulls, to C#
  `ConvexDecompositionDotNet`; there is no byte-level comparison.
- Cache keys are supplied by the caller; there is no eviction policy yet.
- Collision surfaces are checked against reference geometry, not against
  recorded C# collision traces.
