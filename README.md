# Welkin

![License](https://img.shields.io/badge/license-LGPL--3.0-blue)
![Rust](https://img.shields.io/badge/rust-edition%202024-orange)
![Tests](https://img.shields.io/badge/tests-2%2C353%20passing-brightgreen)
[![Docs](https://img.shields.io/badge/docs-flintwave.github.io%2Fwelkin-purple)](https://flintwave.github.io/welkin/)

A Rust implementation of the [OpenSimulator](http://opensimulator.org/) 0.9.3.0
virtual-world server. Wire-compatible with existing viewers and grids.
Built from scratch with no OpenSimulator code.

**Status: pre-alpha.** Both server binaries (grid services and region simulator)
build and run. Viewers connect, avatars walk around, scripts execute, grids
federate over Hypergrid. Many subsystems are still being wired into the running
server; see [ROADMAP.md](ROADMAP.md) for the full picture.

## What is Welkin

Welkin is a virtual-world server that speaks the same protocols as
OpenSimulator 0.9.3.0. It runs your regions and grid services, connects
to existing grids like OSGrid over Hypergrid, and works with the viewers
your users already have.

Your existing grid, content, and viewer, on a server that is easier to
run and harder to break.

The name comes from Old English: "the sky" or "the firmament."

## What works today

Verified through live viewer sessions
([Aurora Viewer](https://aurora-viewer.org), Firestorm) and automated bot
tests:

- Login (XMLRPC and LLSD), circuit establishment, region handshake
- Avatar movement: walk, run, fly, sit, stand
- Chat, instant messaging, group messaging, friends, profiles
- Object creation, editing, duplication, linking, persistence to database
- Permission enforcement on 14 object mutation handlers
- Script upload and execution: LSL compiled to Luau, 206 ll\* builtins,
  236 os\* extension functions with 8 threat levels
- 42 HTTP capability routes, 53 region modules
- Rapier 3D physics (11ms steps), collision events flowing to scripts
- Cross-region teleport, hypergrid teleport, region crossing
- OAR and IAR archive import and export
- Estate administration (11 sub-commands, freeze, eject, god mode)
- 20 NPC functions with real implementations
- Economy: currency module with Gloebit integration
- WebRTC voice via wolfvoice sidecar
- Avatar name resolution from ROBUST UserAccountService

### Still in progress

- PBR materials, Bakes on Mesh, Animesh
- Extended Environment (EEP) day cycles
- Viewer search, media on a prim, animation overrides
- Avatar physics controller (built, not yet wired into the main loop)
- Web-based grid portal (scaffold: most handlers return fake success, and no binary mounts it)
- Terrain brush editing and DCT compression
- Several dozen built-and-tested subsystems still need wiring into the
  running server. The [ROADMAP](ROADMAP.md) tracks every one of them.

## How Welkin differs from OpenSimulator

| | OpenSimulator | Welkin |
|---|---|---|
| Language | C# (.NET) | Rust |
| Async runtime | Threads + async/await (.NET) | tokio |
| Physics | ODE / BulletSim | Rapier 3D |
| Scripting | XEngine (C# codegen) or YEngine | LSL-to-Luau transpiler (mlua) |
| Voice | Vivox / FreeSWITCH | WebRTC (wolfvoice) |
| Admin | Region console + ROBUST web | Web portal (scaffold, not working yet) |
| License | BSD-3-Clause | LGPL-3.0 |

Welkin is an alternative to OpenSimulator, not a replacement. Both projects
serve the same community.

## Viewer support

[Aurora Viewer](https://aurora-viewer.org) is the primary target. Firestorm
(OpenSim build) is the secondary target. Any viewer that speaks the standard
LLUDP and LLSD capability protocols should work.

| Viewer | Status |
|---|---|
| [Aurora Viewer](https://aurora-viewer.org) | Tested. 10-minute soak at 60 fps, zero errors. |
| Firestorm (OpenSim build) | Tested. Login, movement, chat, scripting working. |
| LibreMetaverse bot | 10/10 automated test steps pass. |

## Grid compatibility

Welkin connects to OSGrid and other grids over Hypergrid. In external-grid
mode it talks to an existing ROBUST grid the same way a C# region simulator
would. You can also run it as a standalone grid with its own ROBUST services.

## Quick start

### Requirements

- Rust 1.85+ (edition 2024)
- SQLite for the data stores (MariaDB and PostgreSQL are supported in the code but not yet verified)
- A viewer: [Aurora Viewer](https://aurora-viewer.org) or
  [Firestorm](https://www.firestormviewer.org/) (OpenSim build)

### Build

```sh
git clone https://github.com/FlintWave/welkin.git
cd welkin
cargo build --workspace --release
```

### Run

```sh
# Start grid services
./target/release/welkin-robust --config robust.toml

# In a second terminal, start a region
./target/release/welkin-region --config welkin.toml
```

The web portal and first-run wizard are not working yet. See the
[docs](docs/quick-start.md) for current setup steps.

## Project structure

25 crates in a Cargo workspace, plus a standalone Tauri desktop app:

| Group | Crates | What they do |
|---|---|---|
| Core | `welkin-types`, `-llsd`, `-msg`, `-config`, `-runtime` | Protocol types, LLUDP/LLSD codecs, configuration, logging |
| Network | `welkin-net`, `-caps` | UDP server and per-client state, HTTP capabilities |
| Scene | `welkin-scene`, `-prim`, `-modules` | Scene graph, prim meshing, 53 region modules |
| Physics | `welkin-physics`, `-physics-rapier` | Physics traits, Rapier 3D backend |
| Scripting | `welkin-script-compiler`, `-script-vm`, `-script-api` | LSL parser and Luau transpiler, VM with 442 builtins |
| Services | `welkin-services`, `-data`, `-portal` | 14 ROBUST service handlers, sqlx data layer, web portal (scaffold) |
| Archives | `welkin-archive`, `-migration` | OAR/IAR load/save, C# to Welkin migration tools |
| Testing | `welkin-conformance`, `-capacity`, `-realmwalker` | Protocol replay, capacity benchmarks, Rust bot SDK |
| Binaries | `welkin-region`, `-robust` | Region simulator, grid services server |
| Desktop | `welkin-companion` | Tauri 2 tray app (builds separately) |

Full documentation: [flintwave.github.io/welkin](https://flintwave.github.io/welkin/) (architecture, operator guides, security reviews, API reference).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

LGPL-3.0-or-later. See [LICENSE](LICENSE) for the full text.

Welkin is a clean-room implementation. No code from OpenSimulator or any
GPL-licensed viewer was copied into this repository. The C# OpenSimulator
source is kept in a separate reference directory for protocol study and is
never compiled or linked.
