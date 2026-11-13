## Why

The simulator needs a physics engine. The C# code offers ubODE (BSD) and
BulletSim (zlib) through native libraries, with 19,273 and 23,313 lines of C#
on top of them. Both are open source, so licensing forces nothing. The
question is which engine costs least to carry into Rust and behaves best.

## What Changes

- A `PhysicsScene` and `PhysicsActor` trait pair in Rust mirroring
  `OpenSim/Region/PhysicsModules/SharedBase` (about 35 scene methods and 84
  actor members), so the engine stays replaceable.
- A Rapier backend as the default and only engine built in this change.
- The Second Life vehicle model and the avatar controller ported as
  engine-independent code above the trait.
- A parity spike, run before the simulator exists, that compares Rapier
  trajectories with recordings from C# OpenSim and decides go or no-go.
- The update path that tells viewers about motion keeps its current message
  layout and send thresholds.

ubODE and BulletSim are not carried over. Reasoning is in `design.md`.

## Impact

- New crates: `physics` (traits, vehicle model, avatar controller),
  `physics-rapier`.
- New dependency: `rapier3d` (Apache-2.0).
- Removed from the port: `libubode`, `libBulletSim`, `BulletXNA.dll`, and the
  POS and BasicPhysics modules (BasicPhysics is replaced by a null backend
  used in tests).
- Risk: HIGH. Vehicles and scripted objects in existing regions were tuned
  against ubODE or BulletSim. Any engine change shifts their feel. The spike
  and the tuning tasks exist to bound that.
