## ADDED Requirements

### Requirement: LSL and Luau scripts
The simulator SHALL run scripts written in LSL and in Luau on one virtual machine, with the same script functions available to both.

#### Scenario: Mixed object
- **WHEN** an object contains one LSL script and one Luau script
- **THEN** both run, and a link message sent by one is received by the other

### Requirement: Script state survives save and transfer
A running script's state SHALL be saved with its object and SHALL be restored when the object is loaded again or arrives in another region.

#### Scenario: Region crossing
- **WHEN** an object with a script suspended in the middle of an event crosses into a neighbouring region
- **THEN** the script continues from the same point with the same variable values

### Requirement: Scripts are confined
Each script MUST be limited in memory and processor time, and MUST NOT reach anything outside the script functions.

#### Scenario: Runaway loop
- **WHEN** a script enters an endless loop
- **THEN** other scripts in the region keep receiving their time slices

#### Scenario: Memory limit
- **WHEN** a script exceeds its memory limit
- **THEN** it stops with a stack-heap collision error and the region keeps running

### Requirement: OpenSimulator script extensions
The `os*` functions SHALL be available to scripts under the same permission settings as in OpenSimulator 0.9.3.0.

#### Scenario: Disabled function
- **WHEN** a script calls an `os*` function that the region's configuration does not allow
- **THEN** the call fails with a permission error
