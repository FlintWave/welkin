# Capacity assessor summary

[← Back to docs](README.md) · [Architecture](architecture.md) · [Decisions](decisions.md#c1) · Previous: [Grid services](grid-services-summary.md) · Next: [Grid portal](grid-portal-summary.md)

Change: `openspec/changes/capacity-assessor` (proposal only, no `design.md`).
Crate: `crates/welkin-capacity`. Status: sections 1 and 2 implemented;
sections 3 to 5 are stubs waiting for `region-simulator-core` (Phase 3).

## Purpose

An operator wants to know how many islands a machine can run and how many
users can be on them. Published guidance is old and written for the C#
server, so the crate scans the host and applies sourced coefficients. Once the
simulator exists it will measure instead.

## Commits

| Commit | Content |
|---|---|
| `97f5ebd` | `feat(capacity)`: the whole crate: host scan, coefficients, estimate engine, and the stub modules |
| `880338a` | `chore`: marks the change 13/13 in `tasks.md` |
| `5fc2401` | `docs`: marks capacity-assessor accepted in `ROADMAP.md` |

## Implemented (tasks 1.1 to 2.2)

**Host scan** (`host.rs`). CPU model, cores, threads and frequency. Memory,
with the cgroup v1 and v2 limit read so that `effective_bytes` is the smaller
of host memory and the container limit. Disk total and free space for a path.
Container detection with memory limit and CPU quota.

**Network** (`network.rs`). A stub: public IP, upload and download speed are
optional fields filled by manual configuration. There is no speed test and no
public-address detection yet, so task 1.2 is met only in part.

**Report** (`report.rs`). `HostReport` combines the sections above and
renders as JSON (`to_json`) and as text (`Display`). A command and an API call
for the report are not in the crate yet.

**Coefficients** (`coefficients.rs`). `CoefficientSet` carries a version
(`0.1.0`) and 12 coefficients, each with a unit and a source. It loads and
saves as JSON. The values and sources are those in the proposal table:

| Coefficient | Value | Source |
|---|---|---|
| `memory_per_region_mb` | 1024 | OSgrid wiki tutorial |
| `cpu_cores_per_region` | 1 | OpenSimulator wiki |
| `cpu_idle_region_fraction` | 0.025 | OpenSimulator wiki (0.7.6) |
| `memory_base_mb` | 200 | Hypergrid Business hosting guide |
| `memory_per_1k_prims_mb` | 30 | Hypergrid Business hosting guide |
| `memory_per_user_mb` | 50 | Hypergrid Business hosting guide |
| `max_regions_per_simulator` | 4 | OSgrid forum post |
| `max_regions_per_server` | 16 | OSgrid forum post |
| `max_agents_per_region` | 100 | `Regions.ini.example` |
| `max_prims_per_region` | 15000 | `Regions.ini.example` |
| `client_throttle_max_bps` | 187500 | `OpenSimDefaults.ini` |
| `scene_throttle_max_bps` | 2500000 | `OpenSimDefaults.ini` |

**Estimate engine** (`estimate.rs`). `estimate_capacity` takes a host report,
a coefficient set and a workload profile.

| Profile | Agents per region | Prims per region | Scripts fraction |
|---|---|---|---|
| quiet | 5 | 3000 | 0.1 |
| typical | 20 | 8000 | 0.3 |
| busy | 60 | 14000 | 0.6 |

- CPU limit: (cores minus 1 reserved, floor 0.5) divided by cores per region.
- Memory limit: (effective memory minus base) divided by per-region memory,
  where per-region memory is the region figure plus users and prims for the
  profile.
- Island count is the lowest of those and `max_regions_per_server`.
- Users are islands times agents per region, capped by upload bandwidth
  divided by the client throttle when upload speed is known.
- The result is a range of 80% to 120% of the point value, names the limiting
  resource, and carries `is_measured: false`.

Disk is scanned but does not yet enter the estimate. The scene throttle,
idle-region fraction and per-simulator limit are stored but unused.

**Tests.** Five unit tests in `estimate.rs`: quiet on 8 cores and 32 GB, busy
on 4 cores and 8 GB, single core reserve, bandwidth-limited, and coefficient
round trip.

## Stubbed (tasks 3 to 5)

Each module compiles with its types and a `TODO` marker, and returns empty or
placeholder values.

| Module | Task | Waiting on |
|---|---|---|
| `benchmark.rs` | 3.1 to 3.3: synthetic island, loopback avatars, per-workload runs. Types `BenchmarkConfig` (default 5000 objects, 1000 scripts, 10 avatars) and `BenchmarkResult` exist | `region-simulator-core` |
| `model.rs` | 4.1 to 4.3: `calibrate_from_benchmark` returns an empty `CalibratedModel`; `TradeoffPoint` defined | benchmark, then `cutover` load tests |
| `live.rs` | 5.1 to 5.2: `RegionStatistics` and `CapacityWarning` defined, thresholds 80% and 95%; `check_warnings` returns nothing | running regions, portal |

## Status mismatch to resolve

`880338a` ticks all 13 tasks in `tasks.md`, and the ROADMAP marks the change
accepted. Per the commit message this was a PM decision to accept stubs. The
checkboxes for 1.2 (speed test, public address, command and API), 3.x, 4.x and
5.x describe work that is not done. They should be re-opened or moved into the
Phase 3 change so the task list matches the code.

## Risk

The proposal rates the risk MEDIUM: an estimate that is too high can overload
a machine. The current estimate is built from unmeasured, third-party
figures, so it always reports a range, the limiting resource and
`is_measured: false`. Coefficients are to be calibrated by the `cutover` load
tests.
