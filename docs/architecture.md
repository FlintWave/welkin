# Architecture

Rust port of the OpenSimulator 0.9.3.0 server (grid services and region
simulator). Wire and schema compatibility with the C# server is the contract;
internals are free to change. Licence: GPL-3.0-or-later. The C# reference tree
is read-only at `/var/home/erik/Projects/opensim-0.9.3.0-source/`.

Status: Phase 0, protocol-core and physics-backend are complete. Most crates below are still
empty skeletons (a `lib.rs` with no logic). The table marks which.

## Workspace

Cargo workspace (`Cargo.toml`, edition 2024, `members = ["crates/*"]`). All
crates are prefixed `opensim-`.

| Crate | Role | State |
|---|---|---|
| `opensim-types` | SL protocol enums and bitfields: asset, inventory, folder and wearable types, attachment points, `PermissionMask` | implemented |
| `opensim-llsd` | Wrapper over `aurora-llsd`, plus server-side XML-RPC codec (`src/xmlrpc.rs`) for login and legacy calls | implemented |
| `opensim-msg` | Wrapper over `aurora-msg` (framing, zero-coding, typed messages). `src/objects.rs` adds server-direction object update encoders | implemented |
| `opensim-prim` | Re-exports from `aurora-prim`: prim params, texture entries, extra params, sculpts, volume generation | wrapper only |
| `opensim-net` | Server-side LLUDP circuit: `circuit.rs` (sequence, ack, resend, dedup, ping, timeout) and `throttle.rs` (7 categories, token buckets) | implemented |
| `opensim-config` | TOML configuration with env overrides; `OpenSim.ini` / `Robust.ini` importer | implemented |
| `opensim-runtime` | Logging (`tracing`), Prometheus metrics, health endpoint, graceful shutdown | implemented |
| `opensim-conformance` | Recorder/replayer (HTTP, XML-RPC, LLUDP), `pCampBot` wrapper, test bot (`tests/test_bot.rs`) | implemented |
| `opensim-physics` | `PhysicsScene`/`PhysicsActor` traits, null backend, and solver-independent behaviour: vehicles (`vehicle.rs`), avatar controller (`avatar.rs`), move-to-target/hover/look-at (`behaviors.rs`), keyframed motion (`keyframe.rs`), terse-update thresholds (`terse_update.rs`) | implemented |
| `opensim-physics-rapier` | Rapier 0.36.1 backend: prims and linksets, terrain heightfield, collisions, probes, materials, snapshot/restore; consumes `aurora-prim` geometry | implemented |
| `opensim-caps` | HTTP capabilities | skeleton |
| `opensim-data` | `sqlx` data layer over the existing schema | skeleton |
| `opensim-services` | Robust grid services | skeleton |
| `opensim-scene` | Scene graph | skeleton |
| `opensim-modules` | Region modules (land, permissions, chat, ...) | skeleton |
| `opensim-archive` | OAR/IAR archives | skeleton |
| `opensim-script-compiler`, `-vm`, `-api` | SLua embedding, LSL/Luau, `ll*`/`os*` API | skeleton |
| `opensim-robust` | Binary: grid services | skeleton (`main.rs`) |
| `opensim-region` | Binary: region simulator | skeleton (`main.rs`) |

## Dependency graph (current)

```
aurora-llsd -> opensim-llsd -------------------+
aurora-msg  -> opensim-msg --------------------+--> opensim-net
aurora-prim -> opensim-prim, opensim-physics-rapier
opensim-physics -> opensim-physics-rapier (rapier3d 0.36.1)
opensim-types <- llsd, msg, net, caps, data, scene, physics, archive, scripts

opensim-config, opensim-runtime -> opensim-robust
opensim-data, opensim-caps -> opensim-services -> opensim-robust
opensim-scene, opensim-services -> opensim-modules
opensim-script-compiler -> opensim-script-vm -> opensim-script-api
aurora-net (test bot only) -> opensim-conformance, dev-dep of opensim-msg
```

Layering rule: nothing outside a wrapper crate imports an `aurora-*` crate
directly (the one exception is `opensim-types`, which re-exports shared
definitions, and `opensim-conformance`, which needs `aurora-net` as a client).

## Aurora crates reused

Aurora Viewer (GPL-3.0-or-later) is the acceptance viewer. Its crates are
unpublished (`publish = false`, 0.1.0) and change daily, so they are git
dependencies pinned to commit `3d523b14a17bbc6484b208378c0eef291ad5ac32`
(workspace `Cargo.toml`) and wrapped. Updating the pin is a reviewed change
gated by the replay tests.

| Crate | Used for | Why reused |
|---|---|---|
| `aurora-llsd` | LLSD in XML, binary, notation | Standalone, complete; replaces part of libopenmetaverse |
| `aurora-msg` | Packet framing, zero-coding, message types generated from the Linden message template, reader and writer | Avoids regenerating the codec; same template the viewer uses |
| `aurora-prim` | Prim params, texture entries, extra params, sculpts, geometry generator | Needed for physics shapes and object updates; large to rewrite |
| `aurora-assets` | JPEG 2000, mesh, mesh physics hulls (pulls `openjpeg-sys`, C code) | Declared in the workspace; evaluation pending (audit task 2.5) |
| `aurora-net` | Viewer-side client protocol | Test bot only; never linked into the server |

Aurora is a viewer, so some paths exist in one direction only. Gaps found in
the inventory (protocol-core 1.2) are filled in `opensim-msg` (object update
encoders) and offered upstream.

## Other key dependencies

`tokio` and `axum` (async runtime and HTTP), `sqlx` (MariaDB, PostgreSQL,
SQLite over the existing schema), `rapier3d` 0.36.1 (physics backend, behind
the `opensim-physics` trait), `glam` (vectors, as in Aurora), `tracing`,
`metrics` with `metrics-exporter-prometheus`, `serde`/`toml`.

## Build and deployment

- `Containerfile` (multi-stage, Podman and Docker, static binaries, non-root,
  one image per binary). `compose.yaml` starts database, `robust` and one
  region; `compose.reference.yaml` runs unmodified C# OpenSim as the
  conformance reference.
- A region advertises its external host and UDP port to viewers, so the
  published port must equal the advertised one.
- `scripts/check.sh`: fmt, clippy (`-D warnings`), tests, `cargo deny check`.
  `deny.toml` holds the licence allow-list.

## Verification

1. Unit tests ported with the code.
2. Recorded C# request/reply pairs replayed (`opensim-conformance`,
   `test-data/lludp_session.jsonl`).
3. Test bot on `aurora-net` logs in against a simulator.
4. Fuzz targets in `crates/opensim-msg/fuzz/`.
5. Later: `pCampBot`, physics trajectory comparison against recorded C# runs, Aurora Viewer sessions.

Physics details: `docs/physics-backend-summary.md`.

## Roadmap

See `openspec/ROADMAP.md` for phases 1 to 7, and `docs/decisions.md` for the
decision log.
