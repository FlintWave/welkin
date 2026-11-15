## Context

### Who computes physics

The server does, all of it. A viewer sends `AgentUpdate` (control flags and
camera) and the server moves the avatar. For every moving object or avatar the
server sends `ObjectUpdate` or `ImprovedTerseObjectUpdate` carrying position,
velocity, acceleration, rotation and angular velocity
(`LLClientView.CreateImprovedTerseBlock`). The viewer extrapolates from those
values until the next update arrives. Flexible prims and avatar cloth motion
are visual effects computed in the viewer and never reach the server.

So the engine is invisible on the wire. Compatibility with Aurora Viewer, and
with any other viewer, depends on three things only:

1. the update messages keep their layout,
2. updates are sent under the same conditions (the change thresholds and
   rate limits in `SceneObjectPart` and `ScenePresence`),
3. the values are physically plausible, so extrapolation does not snap.

Mesh physics shapes also need no server-side Havok: the viewer decomposes a
mesh at upload and the asset carries the hull data.

### What Second Life uses

Havok, server-side, under a proprietary licence. It is not available to this
project and nothing in OpenSimulator depends on it.

## Candidates

| Engine | Licence | Language | Verdict |
|---|---|---|---|
| ubODE (current) | BSD | C, via FFI | Rejected |
| BulletSim (current default in `OpenSimDefaults.ini`) | zlib | C++ shim, via FFI | Rejected |
| Rapier 0.36 | Apache-2.0 | Rust | Chosen |
| Jolt | MIT | C++, via FFI | Fallback |

**Why not keep ubODE or BulletSim.** The native libraries are only the
solvers. The engine-specific behaviour (vehicles, avatar movement, linksets,
terrain, mesh handling) lives in the C# layers above them, and that code has
to be rewritten in Rust whichever engine sits underneath. Keeping a native
solver therefore saves almost none of the porting work, and it adds unsafe
FFI, a C or C++ toolchain in every container build, and per-architecture
binaries.

**Why Rapier.** It is pure Rust, so the build is one `cargo build` and the
container can be a static binary. Its feature list, checked on rapier.rs for
this plan, covers rigid bodies, joints, contact events and sensors,
snapshotting and optional cross-platform determinism. Snapshotting maps onto
region save and restore. Scene queries map onto `RaycastWorld`, `BoxProbe`
and `SphereProbe`. Checked against docs.rs for rapier3d 0.36.1 on 2026-11-13: heightfield,
triangle-mesh, convex-hull, convex-decomposition and compound colliders
(`ColliderBuilder`), a CCD solver, a kinematic character controller and a
ray-cast vehicle controller all exist. It depends on `parry3d` ^0.31.1, the
version `aurora-assets` pins, and `parry3d` includes V-HACD.

One existing OpenSimulator fork, GridGate, already lists Rapier as an
experimental per-region engine beside ubODE and BulletSim. Its module is a
reference for mapping the physics interface onto Rapier.

**Why Jolt is only a fallback.** It has a strong reputation for large stacked
scenes, but it is C++ and brings back the FFI and toolchain costs. No
authoritative benchmark was found comparing the two for a server workload;
the one forum comparison found had Rapier ahead in pile tests and Jolt
slightly ahead in one impact test.

## Decisions

1. **Port the vehicle model once, above the trait.** Second Life vehicles are
   not wheeled; they are linear and angular motors with friction timescales,
   hover, banking and vertical attraction, applied as forces each step.
   `ubOde/ODEDynamics.cs` (1,164 lines) is the reference because it is the
   shorter of the two implementations; `BulletS/BSDynamics.cs` (1,800 lines)
   is consulted where they disagree.
2. **Port the avatar controller from `ODECharacter.cs`** (2,273 lines) as a
   capsule driven by forces, not Rapier's kinematic controller, so walking,
   flying, jumping and standing on moving objects keep their current feel.
3. **C# OpenSim is the oracle.** No FFI reference backend is built. Scenes
   are run in C# OpenSim with ubODE, the trajectories are recorded, and the
   Rust backend is compared against them within tolerances.
4. **Collision shapes come from Aurora's geometry code.** `aurora-prim`
   generates prim and sculpt geometry, replacing the C# `Meshing` and
   `ubOdeMeshing` modules (9,739 lines). `aurora-assets` reads mesh assets
   and their physics hulls and already uses `parry3d`, the collision library
   under Rapier. It pins `parry3d` 0.31.1; the version Rapier 0.36 needs must
   be checked, and one of the two moved if they differ.
5. **Single precision.** Regions, including variable-size ones, are small
   enough for `f32`. Revisit only if the spike shows drift at region edges.
6. **Step timing is read from configuration** and defaults to what the C#
   scene loop uses, so time dilation reported to viewers stays meaningful.

## Parity spike: go or no-go

Run before `region-simulator-core`. Each scene is recorded in C# OpenSim and
replayed in a headless Rust harness.

- A box dropped onto terrain comes to rest and stops sending updates.
- A stack of 100 boxes stays standing for 60 seconds.
- 2,000 active bodies in one region hold the configured step time.
- A fast small object does not pass through a thin wall.
- An avatar capsule walks up a 45 degree ramp and stairs, and rides a moving
  platform.
- A car, a boat and an aircraft vehicle follow the recorded path within
  tolerance.
- A 30-prim linkset behaves as one body; a mesh with uploaded hulls collides
  on its hulls.
- A region snapshot restores to an identical state.

If two or more fail and cannot be fixed by tuning, the Jolt fallback is
evaluated behind the same trait.

## Risks

- Vehicle feel will differ. Mitigation: tolerances from the oracle, and
  per-region tuning parameters exposed in configuration.
- Rapier is pre-1.0 and its API changes between releases. Mitigation: pin the
  version and keep Rapier types out of the trait.
