# Physics backend summary

[← Back to docs](index.md) · [Architecture](architecture.md) · [Decisions](decisions.md#r3a) · Previous: [Protocol core](protocol-core-summary.md) · Next: [Grid services](grid-services-summary.md)

Change: `openspec/changes/physics-backend`. Status: complete and accepted
(all tasks in sections 1 to 5 of `tasks.md` are ticked).

## What was built

A physics engine for the region simulator, split in two crates:

- `welkin-physics` defines the `PhysicsScene` and `PhysicsActor` traits and
  holds all behaviour that does not depend on a solver. No engine types appear
  in the trait signatures.
- `welkin-physics-rapier` implements the traits on Rapier 0.36.1
  (`rapier3d`, Apache-2.0, pure Rust). It takes collision geometry from
  `aurora-prim`.

The server is authoritative: viewers send control input and extrapolate from
the position, velocity and rotation values the server sends. The engine is
therefore invisible on the wire, and compatibility depends on update layout,
update conditions and plausible values (see `design.md`).

## Spike and GO decision

The spike (`spike-results.md`, commit `4f6ec64`) found that Rapier 0.36.1
covers heightfield, triangle mesh, convex hull and compound colliders, CCD,
shape casts and a serde snapshot round trip. The result was GO.

- Rapier is Z-up, matching OpenSimulator.
- It handles the ubODE step of 1/54.5454 s without trouble.
- `aurora-prim` and Rapier share a compatible `parry3d`, and trimesh and
  convex hull colliders built from Aurora box and sphere geometry behave
  correctly (commits `841fb59`, `01ce38e`; Rapier bump `1cd81b6`, `83ed653`).
- The replay harness (`osim-conform physics-spike`, `e0ffbe7`) matched
  synthetic box-drop data with a maximum error of 0.20 m and box-rest data
  with about zero error.
- The trajectory recorder (`b86cb0e`, `3c66df8`) exists in C# and Rust, but
  scenes were not recorded from a live C# OpenSim, because no C# toolchain
  was available. The spike therefore validated the harness and the engine's
  features, not full parity against recorded ubODE runs. The larger scenes in
  the design's go/no-go list (100-box stack, 2,000 bodies, vehicles on
  recorded paths) have no automated test in the repository.

Jolt stays the documented fallback behind the same trait; it was not needed.

## Components

### Trait abstraction and null backend

`PhysicsScene` and `PhysicsActor` live in `crates/welkin-physics/src/lib.rs`.
`null.rs` is a backend that stores state and does no simulation, used for
tests of code above the trait (`e41e8ec`).

### Rapier backend

`crates/welkin-physics-rapier/src/lib.rs` (`2e1f10b2`). Changes to actors are
marked dirty and pushed to Rapier in `flush_dirty` before each step.

- Prims and linksets: a linkset is one compound body; shape types are prim,
  convex and none.
- Terrain: a Rapier heightfield collider replaced the first cuboid stand-in
  (`84bd16e`).
- Collisions: collision events, volume detect and top colliders (`e0528f5`).
- Raycasts: ray, box, sphere and plane probes with filter flags.
- Materials: friction, restitution, density and gravity multiplier are
  synced in `flush_dirty` (`73b1052`).
- Snapshot and restore: `snapshot`, `restore` and JSON variants
  (`snapshot_json`, `restore_json`), mapping onto region save and restore
  (`c78921a`).

### Engine-independent behaviour

These modules sit in `welkin-physics`, above the trait, so a change of
solver does not touch them.

- `vehicle.rs`: the LSL vehicle model ported from `ODEDynamics.cs`. Vehicles
  are linear and angular motors with friction timescales, hover, banking and
  vertical attraction, applied as forces each step (`27fc319`).
- `avatar.rs`: the avatar controller ported from `ODECharacter.cs`, a
  force-driven capsule rather than Rapier's kinematic controller, so walking,
  flying, jumping and sitting keep the feel viewers expect (`a5b0eed`).
  This departs from the spike's mitigation note, which suggested the
  kinematic controller; `design.md` decision 2 supersedes it.
- `behaviors.rs`: move-to-target, hover, buoyancy, look-at, and applied
  force, torque and impulse (`09db884`).
- `keyframe.rs`: keyframed motion (`fb7d1f4`).

### Network parity

- `terse_update.rs`: the change thresholds and rate limits from
  `SceneObjectPart` and `ScenePresence`, so a Rust region sends
  `ImprovedTerseObjectUpdate` under the same conditions as C# (`a3d2c24`).
  The object update encoders themselves are in `welkin-msg` (`7241c49`).
- `tests/smoothness_check.rs`: checks that walking, flying, vehicle and
  falling-object update streams do not produce snapping jumps when
  extrapolated (`fb8a150`). This is a test-level check; no live Aurora
  Viewer session is recorded in the repository.

## Test coverage

23 tests pass in the two crates (`cargo test -p welkin-physics
-p welkin-physics-rapier`), not the 20 first reported:

| File | Tests |
|---|---|
| `crates/welkin-physics/src/terse_update.rs` | 15 |
| `crates/welkin-physics/tests/smoothness_check.rs` | 5 |
| `crates/welkin-physics-rapier/tests/spike_aurora_colliders.rs` | 3 |

The Rapier backend, vehicle, avatar, behaviour and keyframe modules have no
unit tests of their own; they are covered only indirectly. This is the main
gap to close before the region simulator depends on them.

## Risks carried forward

- Vehicle feel will differ from ubODE. Tolerances and per-region tuning
  parameters are still to be set against recorded C# trajectories.
- Rapier is pre-1.0; the version is pinned and Rapier types stay out of the
  trait.
