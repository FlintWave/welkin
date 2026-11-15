# Physics Spike Results: Rapier 0.36.1

## Decision

**GO** — Rapier 0.36.1 is suitable for the OpenSimulator physics backend.

## What we tested

1. **Rapier feature coverage (1.1):** Heightfield, triangle mesh, convex hull, compound colliders, CCD, shape casts, and snapshot serialization round-trips all work in rapier3d 0.36.1 with enhanced-determinism and serde-serialize features.

2. **Trajectory recording infrastructure (1.2):** Built `TrajectoryRecorder.cs` for C# OpenSim ubODE that records per-step physics state (position, velocity, orientation, angular velocity) as line-delimited JSON. Corresponding Rust reader in `welkin-conformance`. Actual recording of scenes deferred to when C# toolchain is available.

3. **Headless replay harness (1.3):** Built `osim-conform physics-spike` command that loads trajectory recordings, sets up equivalent Rapier scenes (ground plane, dynamic bodies), steps the simulation, and compares positions. Validated with synthetic box-drop (142 frames, max error 0.20m) and box-rest (10 frames, ~0 error) test data.

4. **Aurora geometry compatibility (1.4):** Confirmed `aurora-prim` and `rapier3d`/`parry3d` share compatible `parry3d` version. Successfully built trimesh and convex hull colliders from Aurora-generated box and sphere prim geometry. Rapier physics step with Aurora geometry: box falls and stops at ground plane correctly.

## Key findings

- Rapier's Z-up coordinate system matches OpenSimulator's Z-up convention (gravity = -9.8 on Z axis)
- ubODE timestep is 1/54.5454 ≈ 18.33ms; Rapier handles this non-standard dt without issues
- `ColliderBuilder::trimesh` and `convex_hull` both accept Aurora prim vertex data directly
- `SoftBodySet` is required in the Rapier 0.36 pipeline step signature
- `BroadPhaseBvh` is the correct broad phase for this version
- Enhanced determinism feature works correctly

## Risks and mitigations

- **Collision behavior differences:** ubODE and Rapier use different solvers; some divergence in stacking stability and contact response is expected. Mitigation: tune materials (friction, restitution) per-scene and use tolerance-based comparison rather than exact match.
- **Vehicle dynamics:** `ODEDynamics` uses ODE-specific motor/spring APIs. Mitigation: reimplement as engine-independent behavior on top of Rapier forces/impulses.
- **Avatar capsule controller:** `ODECharacter` interacts directly with ODE. Mitigation: implement as a kinematic controller using Rapier's character controller module.

## Artifacts

- `crates/welkin-conformance/src/trajectory.rs` — Trajectory reader
- `crates/welkin-conformance/src/spike.rs` — Rapier replay harness
- `crates/welkin-conformance/test-data/` — Synthetic test trajectories
- `crates/welkin-physics-rapier/tests/spike_aurora_colliders.rs` — Aurora geometry tests
- `OpenSim/Region/PhysicsModules/SharedBase/TrajectoryRecorder.cs` — C# recorder (in opensim-0.9.3.0-source)
