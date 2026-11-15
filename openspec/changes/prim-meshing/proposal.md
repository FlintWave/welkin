## Why

Every prim in the world needs a physics collider. The C# code generates
triangle meshes from prim parameters (`PrimMesher.cs`, 2,159 lines in
`Meshing/Meshmerizer`), handles sculpt maps (`SculptMesh.cs`), decodes
mesh assets, and runs convex decomposition (`ConvexDecompositionDotNet`,
5,701 lines) to produce physics hulls. The Rust side already has
`aurora-prim` which generates volume geometry (faces and vertices) from
the same prim parameters, and `parry3d` (pulled in via `rapier3d`) which
includes V-HACD for convex decomposition.

The task is to bridge these: take `aurora-prim`'s `VolumeMesh` output
and produce Rapier-compatible colliders, with convex decomposition for
complex shapes and mesh asset decoding for mesh objects.

## What Changes

### Prim-to-collider pipeline (`welkin-prim`)

- Given a `SceneObjectPart`'s VolumeParams and physics shape type, produce
  a `SharedShape` (Rapier collider geometry) using aurora-prim for vertex
  generation and parry3d for shape construction.
- Shape types: box and sphere use analytic colliders; everything else goes
  through the triangle mesh or convex hull path.
- LOD selection: physics uses the lowest LOD that preserves the collision
  surface. Match C# `Meshmerizer`'s LOD table.

### Sculpt maps

- Decode sculpt texture (JPEG2000 via aurora-assets or image crate) into
  a displacement map, feed it to `aurora-prim::generate_sculpt`, produce
  a triangle mesh collider.
- Sculpt types: sphere, torus, plane, cylinder, plus invert/mirror flags.

### Mesh assets

- Decode the mesh asset format (LLSD header + compressed LOD data) to
  extract physics mesh or highest-LOD visual mesh when no physics mesh
  is provided.
- Produce a triangle mesh or convex hull collider from the decoded data.
- This may use `aurora-assets` if its mesh decoder covers the format;
  otherwise a standalone decoder in `welkin-prim`.

### Convex decomposition

- For prims and meshes that need convex colliders, run V-HACD (from
  `parry3d::transformation::vhacd`) on the triangle mesh to produce
  a compound convex-hull collider.
- Tuning: match C# `ConvexDecompositionDotNet` output quality. The
  parameters (max hulls, resolution, concavity threshold) are calibrated
  against a test set of common prim shapes.

### Integration with physics

- The `PhysicsScene` trait's `add_prim` path calls the meshing pipeline
  to get a `SharedShape`, then hands it to Rapier. This replaces the
  placeholder shapes used in the Phase 1 spike.
- Linkset compound colliders: each part's shape is combined into a single
  compound collider for the group.

## Impact

- Modifies: `welkin-prim` (new meshing module), `welkin-physics` (shape
  integration), `welkin-physics-rapier` (real colliders replace placeholders).
- New dependency: `image` crate for sculpt texture decoding (if not using
  aurora-assets). `parry3d` is already present via rapier3d.
- Risk: MEDIUM. Incorrect colliders cause objects to fall through floors
  or avatars to get stuck. Tested against a reference set of prims with
  known C# collision behavior. Convex decomposition is CPU-intensive;
  shapes are cached after first computation.
