## ADDED Requirements

### Requirement: One-command install
The project SHALL provide an installer that brings up the database, the grid services and the portal with one command and reports the portal's address.

#### Scenario: Upgrade
- **WHEN** the installer is run again on a machine with an existing world
- **THEN** the software is upgraded and accounts, inventory, assets and regions are unchanged

### Requirement: First-run wizard
The portal SHALL lead a new operator from an empty install to a running island without editing a configuration file.

#### Scenario: First launch
- **WHEN** the server is launched for the first time and the portal is opened
- **THEN** the wizard is shown, and after it is completed the portal opens on its home page instead

#### Scenario: Standalone world
- **WHEN** the operator completes the wizard choosing standalone mode and one empty island
- **THEN** the island is running and the wizard shows the address to enter in a viewer

#### Scenario: Unreachable port
- **WHEN** a region's UDP port cannot be reached from outside
- **THEN** the wizard reports which port failed and does not mark the step complete

### Requirement: Map placement
The portal SHALL let an operator place, move and remove islands on a grid map and MUST refuse coordinates that are occupied.

#### Scenario: Occupied coordinate
- **WHEN** an island is dropped on coordinates already used by another region
- **THEN** the placement is refused and the nearest free coordinates are offered

### Requirement: Licensed content import
The portal SHALL import region archives, and a catalogue entry MUST carry a verified licence that is shown to the operator before import.

#### Scenario: Catalogue import
- **WHEN** the operator selects a catalogue archive
- **THEN** its licence and source are shown, and the archive is downloaded and loaded only after the operator accepts

### Requirement: Protected administration
Portal administration MUST require an authenticated staff session, and every change to accounts or regions SHALL be written to an audit log.

#### Scenario: Unauthenticated request
- **WHEN** a region-control request arrives without a valid staff session
- **THEN** it is refused and the attempt is logged

### Requirement: Desktop companion
The project SHALL provide an optional desktop application that shows the world's status and opens the portal, and the server MUST run without it.

#### Scenario: Open portal
- **WHEN** the operator chooses "Open portal" from the tray menu or the control window
- **THEN** the default browser opens the portal

#### Scenario: Desktop without a tray
- **WHEN** the desktop provides no tray
- **THEN** the control window and its launcher entry still give status and the same actions

#### Scenario: Status
- **WHEN** an island stops unexpectedly
- **THEN** the status indicator changes and a desktop notification names the island
