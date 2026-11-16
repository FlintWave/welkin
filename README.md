# Welkin

![Status](https://img.shields.io/badge/status-alpha-orange)
![License](https://img.shields.io/badge/license-LGPL--3.0-blue)
![Tests](https://img.shields.io/badge/tests-2%2C688%20passing-brightgreen)
[![Docs](https://img.shields.io/badge/docs-flintwave.github.io%2Fwelkin-purple)](https://flintwave.github.io/welkin/)

An OpenSimulator-compatible virtual world server, written in Rust.
Connect with [Aurora Viewer](https://aurora-viewer.org),
[Firestorm](https://www.firestormviewer.org/), or any viewer that speaks the
OpenSim 0.9.3.0 protocols. Same protocols, different engine.

If you've run a Second Life or OpenSimulator region before, you know the idea.
If you haven't: it's a 3D space you host yourself where people can walk around,
build things, chat, and run scripts.

The name is Old English for "the sky."

## Try it out

This is alpha software. Login works, you can walk and fly around, chat, place
objects, run scripts, shape the terrain, and manage groups. Plenty of stuff is
still missing or stubbed out, and the [CHANGELOG](CHANGELOG.md) doesn't hide
any of it.

## Building

[Rust 1.88+](https://rustup.rs/).

```sh
git clone https://github.com/FlintWave/welkin.git
cd welkin
cargo build --release
```

You'll also need default avatar assets before the first run. These belong to
Linden Lab (CC BY-SA 2.5) so Welkin can't include them. Grab `bin/assets` from
an OpenSimulator 0.9.3.0 install and drop it in `assets/` next to the binary.
[Details here.](docs/quick-start.md#default-assets)

## Running

You'll need a viewer to connect: [Aurora Viewer](https://aurora-viewer.org) or
the [Firestorm](https://www.firestormviewer.org/) OpenSim build.

Grid services first, then the region. Two terminals:

```sh
./target/release/welkin-robust --config robust.toml
./target/release/welkin-region --config welkin.toml
```

Make yourself an account:

```text
create-user Alice Smith
```

Set your viewer's login address to `http://127.0.0.1:8002/`, log in, and you
should be standing in a region. The [quick start guide](docs/quick-start.md)
covers viewer setup, importing archived region content, and managing your grid.

## Heads up

- No access-control gates on the service endpoints yet. Keep it on localhost
  or behind a firewall. The development config has `AllowCreateUser` turned on,
  so anyone who can reach the port can make accounts.
- No teleport between regions. Single-region only for now.
- SQLite is the only tested database. MariaDB and PostgreSQL schemas exist in
  the code but have never been run.
- The web portal is a scaffold. Most of its handlers do nothing. Create
  accounts from the region console instead.
- Many LSL script functions compile and run but aren't wired to the region, so
  they log a message and return nothing. The CHANGELOG lists which ones.
- Module-initiated messages (alerts, kicks, dialogs, appearance updates) are
  silently dropped at runtime. The plumbing exists but isn't connected yet.

## Docs

[flintwave.github.io/welkin](https://flintwave.github.io/welkin/) covers
setup, configuration, database options, architecture, and the roadmap.

## Contributing

Help is welcome. See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

LGPL-3.0-or-later. See [LICENSE](LICENSE).

No OpenSimulator or GPL viewer source code is in this repo.
