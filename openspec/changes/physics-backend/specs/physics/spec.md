## ADDED Requirements

### Requirement: Server-authoritative physics
The simulator SHALL compute all rigid-body and avatar motion and SHALL report it to viewers in `ObjectUpdate` and `ImprovedTerseObjectUpdate` messages whose layout and send conditions match OpenSimulator 0.9.3.0.

#### Scenario: Object comes to rest
- **WHEN** a physical object stops moving
- **THEN** the simulator sends one update with zero velocity and sends no further updates for that object until it moves again

#### Scenario: Viewer extrapolation
- **WHEN** a physical object moves at constant velocity
- **THEN** each update carries position, velocity, acceleration, rotation and angular velocity consistent with the engine state at the time it is sent

### Requirement: Replaceable physics engine
The simulator SHALL reach the physics engine only through the `PhysicsScene` and `PhysicsActor` traits, and those traits MUST NOT expose types from any engine.

#### Scenario: Null backend
- **WHEN** the simulator is started with the null backend
- **THEN** it runs and serves viewers with objects that do not move under physics

### Requirement: Vehicle and avatar behaviour independent of the engine
The Second Life vehicle model and the avatar movement controller SHALL be implemented above the physics traits.

#### Scenario: Scripted vehicle
- **WHEN** a script sets a vehicle type and motor parameters on a physical object
- **THEN** the object's path stays within the recorded tolerance of the same script run on C# OpenSim with ubODE

### Requirement: Region physics state can be saved and restored
The physics backend SHALL produce a snapshot of a region's state and SHALL restore it to an identical state.

#### Scenario: Restart
- **WHEN** a region is snapshotted, stopped and restored
- **THEN** every body has the position, rotation and velocity it had at the snapshot
