## ADDED Requirements

### Requirement: Host assessment
The assessor SHALL scan the host and report how many islands it can run and how many users it can serve in total across them.

#### Scenario: Wizard step
- **WHEN** the operator reaches the capacity step of the wizard
- **THEN** the assessor shows a number of islands and a total number of users for quiet, typical and busy workloads, each as a range

### Requirement: Limiting resource is named
Every assessment SHALL state which of CPU, memory, bandwidth or disk limits the result.

#### Scenario: Slow uplink
- **WHEN** the host has ample CPU and memory and a slow upload link
- **THEN** the user total is limited by bandwidth and the report says so

### Requirement: Measured before estimated
The assessor SHALL use a benchmark run on the host when the simulator is available, and MUST label any result that rests on published rules of thumb as unmeasured.

#### Scenario: No benchmark available
- **WHEN** the benchmark cannot run
- **THEN** the report is marked unmeasured and lists the source of each figure used

### Requirement: Container limits are respected
The assessor MUST base its result on the resources available to the server process, not on the whole machine, when a limit applies.

#### Scenario: Memory limit
- **WHEN** the server runs in a container limited to 4 GB on a 64 GB host
- **THEN** the assessment uses 4 GB

### Requirement: Live correction
The portal SHALL compare running regions with the assessment and SHALL warn the operator before a measured limit is reached.

#### Scenario: Approaching limit
- **WHEN** measured frame time on an island stays above the warning threshold
- **THEN** the portal shows a warning naming the island and the resource
