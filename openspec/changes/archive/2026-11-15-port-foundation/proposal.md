## Why

The port needs a home, a build, a way to run in containers and a way to prove
that a Rust component behaves like the C# one it replaces. Without the last of
these, every later phase is judged by eye.

## What Changes

- A new git repository holding a Cargo workspace. This OpenSpec directory
  moves into it. The C# tree stays read-only as the reference.
- Licence GPL-3.0-or-later. A notice file carries the OpenSimulator BSD
  notice and the notices of the reused Aurora Viewer crates.
- Workspace layout (crate names provisional): `types`, `llsd`, `msg` and
  `prim` (wrappers over the pinned Aurora crates), `net`, `caps`, `data`,
  `services`, `scene`, `physics`, `physics-rapier`, `script-compiler`,
  `script-vm`, `script-api`, `modules`, `archive`, and two binaries,
  `robust` (grid services) and `region` (simulator).
- Configuration in TOML with environment overrides, replacing Nini INI files,
  plus an importer that reads an existing `OpenSim.ini` and `Robust.ini`.
- Structured logs (`tracing`), Prometheus metrics, health endpoints and
  graceful shutdown, replacing log4net and the stats console.
- Container images built from one multi-stage `Containerfile` that works with
  Podman and Docker: static binaries, non-root user, one image per binary. A
  compose file starts a database, `robust` and one region.
- CI: format, lint, tests, `cargo deny` for licences and advisories, image
  build.
- A conformance harness: a recorder that captures request and reply pairs
  from the running C# server, a replayer that checks the Rust server against
  them, and a wrapper that runs `pCampBot` against either server.

## Impact

- No dependency on Mono.Addins, Nini, log4net or Prebuild.
- Risk: LOW. No runtime behaviour is ported in this change.
- Networking note for containers: a region advertises its external host name
  and UDP port to viewers, so the published port must equal the advertised
  one. The compose file and documentation must make that explicit.
