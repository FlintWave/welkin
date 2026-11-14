## 1. Spike

- [x] 1.1 Confirm in Rapier 0.36: heightfield, triangle mesh, convex hull and compound colliders, CCD, shape casts, snapshot round trip
- [x] 1.2 Add a trajectory recorder to C# OpenSim and record the spike scenes with ubODE
- [x] 1.3 Build the headless Rust harness and replay the scenes on Rapier
- [x] 1.4 Check that `aurora-assets` and Rapier agree on a `parry3d` version; build one prim and one mesh collider from Aurora geometry
- [x] 1.5 Write up results and record go or no-go in this change

## 2. Traits

- [x] 2.1 Define `PhysicsScene` and `PhysicsActor` from `SharedBase`, with no engine types in the signatures
- [x] 2.2 Null backend for tests

## 3. Rapier backend

- [x] 3.1 Prims, linksets as compound bodies, physics shape types (prim, convex, none)
- [x] 3.2 Terrain heightfield and water level
- [x] 3.3 Collision events, volume detect, top colliders
- [x] 3.4 Ray, box, sphere and plane probes with filter flags
- [x] 3.5 Materials: friction, restitution, density, gravity multiplier
- [x] 3.6 Snapshot and restore

## 4. Engine-independent behaviour

- [ ] 4.1 Vehicle model from `ODEDynamics.cs`
- [ ] 4.2 Avatar controller from `ODECharacter.cs`, including sit
- [ ] 4.3 Move-to-target, hover, buoyancy, look-at, applied force, torque and impulse
- [ ] 4.4 Keyframed motion

## 5. Network parity

- [ ] 5.1 Port the terse-update thresholds and rate limits with tests against recorded C# output
- [ ] 5.2 Aurora Viewer check: walking, flying, vehicles and falling objects show no snapping
