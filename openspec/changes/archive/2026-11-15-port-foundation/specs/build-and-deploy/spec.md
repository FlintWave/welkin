## ADDED Requirements

### Requirement: Container images
The project SHALL build one container image per server binary from a single `Containerfile`, and each image MUST run as a non-root user.

#### Scenario: Local grid
- **WHEN** the compose file is started on a machine with Podman or Docker
- **THEN** a database, the grid services and one region start and report healthy

### Requirement: Configuration import
The project SHALL read an existing OpenSimulator INI configuration and produce an equivalent TOML configuration.

#### Scenario: Existing grid
- **WHEN** the importer is given a working `Robust.ini`
- **THEN** it writes a TOML file and lists every setting it could not translate

### Requirement: Conformance replay
The project SHALL provide a tool that replays recorded C# server exchanges against a Rust server and reports each difference.

#### Scenario: Matching reply
- **WHEN** a recorded request is replayed and the reply matches after normalising timestamps and generated identifiers
- **THEN** the exchange is reported as passing

### Requirement: Licence gate
CI SHALL fail when a dependency has a licence outside the project's allow-list.

#### Scenario: Disallowed dependency
- **WHEN** a pull request adds a dependency under a licence that is not on the allow-list
- **THEN** the licence check fails and names the dependency
