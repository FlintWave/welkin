## ADDED Requirements

### Requirement: Connector compatibility
Each Rust grid service SHALL accept the requests and return the replies that the corresponding OpenSimulator 0.9.3.0 service does, so that an unmodified C# simulator can use it.

#### Scenario: Mixed grid
- **WHEN** a C# simulator is configured to use the Rust grid services
- **THEN** a viewer can log in, load inventory, rez an object and teleport

### Requirement: Shared database schema
The grid services SHALL read and write the existing OpenSimulator schema without a conversion step.

#### Scenario: Switching implementations
- **WHEN** the Rust services are stopped and the C# services are started on the same database
- **THEN** all accounts, inventory and assets are present and usable

### Requirement: Credentials are never weakened
The authentication service MUST verify passwords against the stored salted hashes and MUST NOT log credentials or session tokens.

#### Scenario: Wrong password
- **WHEN** a login is attempted with a wrong password
- **THEN** it is refused and the log contains no password or token material

### Requirement: Services can be enabled individually
Each grid service SHALL be switchable on or off in configuration.

#### Scenario: Gradual replacement
- **WHEN** only the asset service is enabled in the Rust binary
- **THEN** the other services continue to be served by the C# server
