## 1. Prim colliders

- [x] 1.1 Analytic shapes: box and sphere prims produce BoxShape and BallShape colliders directly
- [x] 1.2 Volume mesh path: VolumeParams through aurora-prim, triangle mesh to TriMeshShape collider
- [x] 1.3 LOD selection: physics LOD table matching C# Meshmerizer thresholds
- [x] 1.4 Shape cache: hash VolumeParams to avoid redundant mesh generation

## 2. Sculpt maps

- [x] 2.1 Sculpt texture decode: JPEG2000 sculpt map to displacement grid
- [x] 2.2 Generate sculpt mesh via aurora-prim, produce TriMeshShape collider
- [x] 2.3 Sculpt types and flags: sphere, torus, plane, cylinder, invert, mirror

## 3. Mesh assets

- [x] 3.1 Mesh asset decoder: LLSD header, decompress LOD data, extract physics or visual mesh
- [x] 3.2 Produce TriMeshShape or ConvexPolyhedron from decoded mesh vertices and indices

## 4. Convex decomposition

- [x] 4.1 V-HACD pipeline: triangle mesh in, compound convex hull collider out, via parry3d
- [x] 4.2 Parameter tuning: max hulls, resolution, concavity threshold calibrated against C# output
- [x] 4.3 Decomposition cache: store results keyed by shape hash

## 5. Integration

- [x] 5.1 Wire meshing into PhysicsScene::add_prim, replacing placeholder shapes
- [x] 5.2 Linkset compound colliders: combine per-part shapes with relative transforms
- [x] 5.3 Shape type switching: prim, convex, none, mesh — runtime shape changes

## 6. Proof

- [x] 6.1 Collider tests: reference prims (box, cylinder, torus, sculpt, mesh) match C# collision surfaces
- [x] 6.2 Drop test: objects with real colliders land correctly on terrain and on other objects
- [x] 6.3 Performance: meshing 1000 mixed prims completes within 5 seconds
- [x] 6.4 Second review of mesh asset parsing (untrusted input from user uploads) (CONDITIONAL PASS: CRITICAL-1 index OOB + HIGH-1 decompression bomb — fixes dispatched)
