# Welkin

![Status](https://img.shields.io/badge/status-alpha-orange)
![License](https://img.shields.io/badge/license-LGPL--3.0-blue)
![Rust](https://img.shields.io/badge/rust-edition%202024-orange)
![Tests](https://img.shields.io/badge/tests-2%2C652%20passing-brightgreen)
[![Docs](https://img.shields.io/badge/docs-flintwave.github.io%2Fwelkin-purple)](https://flintwave.github.io/welkin/)

A Rust implementation of the [OpenSimulator](http://opensimulator.org/) 0.9.3.0
virtual-world server. It speaks the same viewer and grid protocols. Built from
scratch with no OpenSimulator code.

> **Alpha release (v0.1.0-alpha).** Welkin builds, runs, and lets a viewer log in
> to a single-region grid on SQLite. It is not ready for a production grid. Read
> [What works](#what-works), [What does not work yet](#what-does-not-work-yet)
> and [Known limits](#known-limits) before you run it.

## What is Welkin

Welkin is a virtual-world server that aims to be compatible with
OpenSimulator 0.9.3.0. It has two programs: `welkin-robust` runs the grid
services (login, inventory, assets, groups and others), and `welkin-region` runs
a region that viewers connect to. The goal is your existing grid, content and
viewer on a server that is easier to run and harder to break.

Welkin is an alternative to OpenSimulator, not a replacement. Both projects
serve the same community. The name comes from Old English: "the sky" or "the
firmament."

## What works

The release build produces both binaries, and the workspace test suite passes:
2,652 tests, 0 failures, 2 ignored.

Login, movement, chat and basic scripting were tried in a live session with the
Firestorm OpenSim build and with a LibreMetaverse bot. An
[Aurora Viewer](https://aurora-viewer.org) connection is reported, but no
recording or log of it is on record. The features below that came later are
tested in code, including end-to-end tests against real services,
but have not been through a live viewer session. The
[viewer compatibility page](docs/viewer-compatibility.md) says which is which.

- Login (XMLRPC and LLSD), region handshake, avatar movement (walk, run, fly,
  sit), jumping and falling on the real terrain
- Chat, instant messages, friends and profiles
- Objects: create, edit, link, take, return and rez from inventory, attachments,
  all saved to the database and loaded again at start
- Terraforming with the viewer's land tools, with real terrain data sent to
  viewers and saved across restarts
- Physics with Rapier 3D, including physical prims and script physics commands
- Scripts: LSL compiled to Luau and run in a sandboxed VM. The compiler handles
  the common LSL patterns; in a sample of 25 real-world scripts, about 60% (15)
  compile. Chat, timers, events, sensors, controls, sound and physics calls work
  at runtime. A threat-level model gates the `os*` functions.
- Estate settings and object permissions, enforced by default
- Groups: create, join, leave, invitations, roles and powers, titles, eject, and
  group chat and group notices (notice attachments are not supported yet)
- OAR and IAR archive import and export
- Account creation from the region console (`create-user`)

Also implemented in code but not yet checked in a live session: Hypergrid
friends, WebRTC voice through the `wolfvoice` sidecar, and the currency module
with Gloebit.

## What does not work yet

- **The web portal is a scaffold.** Most handlers do nothing and no binary
  mounts it. There is no first-run wizard. Create accounts with the console.
- **Many LSL functions exist in the VM but are not connected to the running
  region, so they do nothing at runtime:** `llEmail` (no mail sender),
  `llHTTPRequest`, `llHTTPResponse` and `llRequestURL`, `llRezObject` and
  `llRezAtRoot`, and `llAttachToAvatar` and its detach call. `llRequestAgentData`
  returns fixed placeholder data, not real account data, and `llSetHTTPBody` does
  not exist. A script that calls these runs without an error and gets no result.
- Teleport between regions and Hypergrid teleport, NPC functions and dynamic
  textures are not wired into the running region.
- The LSL compiler fails on some valid scripts. For example, the lexer rejects a
  float literal that starts with a dot, such as `.4`.
- Group notice attachments. Group proposals and votes are out of scope. In group chat, the
  event for a second avatar that joins a session can arrive late.
- Attachments are saved when an avatar logs out, not on a timer, so a region crash
  loses attachment changes made since login.
- PBR materials, Bakes on Mesh, Animesh, Extended Environment (EEP) day cycles,
  viewer search, and media on a prim.
- Separate-host setups: the grid's private service port listens on `127.0.0.1`
  by default, so a region on another machine cannot reach it until you change
  the bind address.

## Known limits

- The LibreMetaverse bot test passes 10/10 on a fresh region; on a region that already holds objects from earlier runs the touch step can fail. The cause is under investigation; probably the test picks a stale prim.
- **Run Welkin on a trusted network only.** The grid's authentication and account
  service endpoints have no access-control gates yet. For example, there is no
  `AllowSetPassword` check, so anyone who can reach the private service port can
  change account data. The portal handlers do not validate their input. Keep the
  private port on `127.0.0.1` (the default) or behind a firewall, and do not put
  an alpha grid on the open internet.
- **SQLite is the only database that has been run.** MariaDB and PostgreSQL are
  supported in the code but have never been tried, and some SQL is known to need
  work. See [Database setup](docs/database-setup.md).
- The container stack in `docker-compose.prod.yml` uses MySQL, so it is
  untested for the same reason.
- Several features above were tested in code only. Expect bugs in them.
- Account creation is off by default in the grid (`AllowCreateUser`). The
  development `robust.toml` in this repository turns it on; do not copy that
  setting into a grid that others can reach.

## Quick start

You need Rust 1.88 or later and a viewer
([Aurora Viewer](https://aurora-viewer.org) or
[Firestorm](https://www.firestormviewer.org/), OpenSim build). The steps below
use SQLite and one region on one machine.

```sh
git clone https://github.com/FlintWave/welkin.git
cd welkin
cargo build --release
```

That builds `target/release/welkin-robust` and `target/release/welkin-region`.
Start the grid services first, then the region, each in its own terminal:

```sh
./target/release/welkin-robust --config robust.toml
./target/release/welkin-region --config welkin.toml
```

At the region's `welkin>` prompt, create an account. The console asks for the
password twice and does not show it:

```text
create-user Alice Smith
```

Then point your viewer at the login URI `http://127.0.0.1:8002/` and log in as
`Alice Smith`. The [quick start guide](docs/quick-start.md) has the details,
including Firestorm's grid manager.

To make an avatar the estate owner, set `[Estate] EstateOwner` in `welkin.toml`
to the avatar's UUID. Without an owner nobody can terraform or use the estate
tools. See [Regions, estate and users](docs/regions-estate-users.md).

## Documentation

The book at [flintwave.github.io/welkin](https://flintwave.github.io/welkin/) has
two parts:

- **Operator guide**: build, configure and run a grid, set up the database, run
  regions and estates, deploy containers, and migrate from OpenSimulator.
- **Agent build guide**: architecture, crate structure, how to add a module,
  testing, the OpenSpec workflow and the task system, for people and AI agents
  who develop Welkin.

The sources are in [docs/](docs/). [ROADMAP.md](ROADMAP.md) lists what is in this
release and what is paused.

## Project structure

25 crates in a Cargo workspace, plus a standalone Tauri desktop app:

| Group | Crates | What they do |
|---|---|---|
| Core | `welkin-types`, `-llsd`, `-msg`, `-config`, `-runtime` | Protocol types, LLUDP/LLSD codecs, configuration, logging |
| Network | `welkin-net`, `-caps` | UDP server and per-client state, HTTP capabilities |
| Scene | `welkin-scene`, `-prim`, `-modules` | Scene graph, prim meshing, region modules |
| Physics | `welkin-physics`, `-physics-rapier` | Physics traits, Rapier 3D backend |
| Scripting | `welkin-script-compiler`, `-script-vm`, `-script-api` | LSL parser and Luau transpiler, VM, builtin tables |
| Services | `welkin-services`, `-data`, `-portal` | ROBUST service handlers, sqlx data layer, web portal (scaffold) |
| Archives | `welkin-archive`, `-migration` | OAR/IAR load and save, C# to Welkin migration tools |
| Testing | `welkin-conformance`, `-capacity`, `-realmwalker` | Protocol replay, capacity benchmarks, Rust bot SDK |
| Binaries | `welkin-region`, `-robust` | Region simulator, grid services server |
| Desktop | `welkin-companion` | Tauri 2 tray app (builds separately) |

## Contributing

Contributions are welcome, especially on the paused work in the roadmap. See
[CONTRIBUTING.md](CONTRIBUTING.md).

## License

LGPL-3.0-or-later. See [LICENSE](LICENSE) for the full text.

Welkin is a clean-room implementation. No code from OpenSimulator or any
GPL-licensed viewer was copied into this repository. The C# OpenSimulator source
is kept in a separate reference directory for protocol study and is never
compiled or linked.
