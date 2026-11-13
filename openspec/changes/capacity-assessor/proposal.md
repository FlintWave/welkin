## Why

An operator needs to know, before placing islands, how many the machine
can run and how many people can be on them at once. Published guidance is
old and written for the C# server, so it cannot be reused as it stands.

## What Changes

- **Host scan.** CPU model, cores and per-core speed; memory, including
  container limits; disk free space and write speed; network up and down
  speed and whether the address is public; other load on the machine.
- **On-host benchmark.** A short run of the real simulator in a synthetic
  mode: one island with a set number of objects, scripts and simulated
  avatars connected over loopback. It measures frame time, memory per
  island, memory and CPU per avatar, and bytes sent per avatar. This
  measures the machine, so the answer does not rest on a table of
  processor names.
- **Capacity model.** Islands and users are each limited by the scarcest
  of CPU, memory, bandwidth and disk, with a stated reserve. The result is
  given for three workloads (quiet, typical, busy), names the limiting
  resource, and shows the trade-off between more islands and more users.
- **Output**: "this machine supports N islands with M users in total",
  with a range, shown in the wizard and available as a command and an API
  call. Coefficients live in a versioned file and are reported with the
  result.
- **Live refinement.** Once regions run, measured statistics (the same
  ones the C# server reports: frame time, physics time, script time,
  agents, active objects and scripts, packets, unacknowledged bytes)
  replace the benchmark figures, and the portal warns before a limit is
  reached.

Starting points, to be replaced by measurement, with their sources:

| Figure | Source |
|---|---|
| About 1 GB of memory and one core per region | OSgrid wiki tutorial |
| One core per regularly used region, minimum two; an idle region about 2.5% of a core (version 0.7.6) | OpenSimulator wiki |
| 200 MB base, plus textures, plus 30 MB per 1,000 prims, plus 50 MB per user | Hypergrid Business hosting guide |
| 4 regions per simulator, 16 per server | OSgrid forum post |
| `MaxAgents = 100`, `MaxPrims = 15000` per region | `Regions.ini.example` in this tree |
| `client_throttle_max_bps = 187500`, `scene_throttle_max_bps = 2500000` | `OpenSimDefaults.ini` in this tree |

No per-avatar bandwidth figure was found; the throttle values bound it.

## Impact

- The benchmark needs `region-simulator-core`. Before that exists, the
  assessor reports the host scan and a clearly labelled estimate from the
  table above.
- Coefficients are calibrated by the load tests in `cutover`.
- Risk: MEDIUM. An estimate that is too high leads an operator to
  overload a machine. The result always shows a range and the limiting
  resource, never a single unqualified number.
