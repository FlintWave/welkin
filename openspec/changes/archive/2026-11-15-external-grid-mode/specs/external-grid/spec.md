## ADDED Requirements

### Requirement: Grid profiles
The region simulator SHALL take every grid service address and connector choice from a grid profile, and SHALL ship profiles for a standalone world, a private grid, OSgrid and a custom grid.

#### Scenario: Switching grid
- **WHEN** an operator changes a region's profile from `own-grid` to `custom` and restarts it
- **THEN** the region registers with the grid named in the custom profile and no code change is needed

### Requirement: OSgrid profile from OSgrid's own configuration
The OSgrid profile SHALL be generated from the configuration files in OSgrid's published distribution.

#### Scenario: Address change
- **WHEN** OSgrid publishes a distribution with changed service addresses and the importer is run on it
- **THEN** the profile carries the new addresses

### Requirement: Compatible region registration
The region simulator SHALL register, deregister and exchange neighbour and agent-transfer messages in the form an OpenSimulator 0.9.3.0 region uses.

#### Scenario: Teleport from a C# region
- **WHEN** an avatar teleports from a C# region on the same grid to a Rust region
- **THEN** the avatar arrives with appearance, attachments and running scripts intact

### Requirement: Safe by default on shared grids
A profile for a grid the operator does not run MUST be disabled until the operator enables it, and remote inventory or asset writes MUST fail closed on any protocol error.

#### Scenario: Unexpected reply
- **WHEN** a remote inventory write returns a reply the connector cannot parse
- **THEN** the write is reported as failed and is not retried in a different form
