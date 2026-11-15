## ADDED Requirements

### Requirement: Open-source components only
The project SHALL ship only code and data under an open-source licence recorded in the licence audit, and MUST NOT depend on a proprietary library or service.

#### Scenario: Unknown provenance
- **WHEN** a data file has no established licence
- **THEN** it is excluded from releases until the audit records one

#### Scenario: Proprietary voice service
- **WHEN** the server is built
- **THEN** it contains no Vivox client code

### Requirement: Project licence
The project SHALL be licensed GPL-3.0-or-later, and every dependency MUST be under a licence compatible with GPLv3.

#### Scenario: Reused Aurora crate
- **WHEN** a crate from Aurora Viewer is added as a dependency
- **THEN** its notices are reproduced in the project's notice file and the Aurora name and logo are not used for the project or its binaries

### Requirement: Upstream attribution
The project SHALL retain the OpenSimulator copyright notice and contributor list for all ported code.

#### Scenario: Release archive
- **WHEN** a release is built
- **THEN** it contains a notice file with the OpenSimulator BSD licence text
