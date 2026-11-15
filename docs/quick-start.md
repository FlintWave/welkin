<!-- SPDX-License-Identifier: LGPL-3.0-or-later -->
# Quick start

[← Back to docs](README.md) · [Architecture](architecture.md) · [Configuration reference](configuration.md)

## Requirements

- Rust 1.85 or later (edition 2024)
- MariaDB, PostgreSQL, or SQLite
- A viewer: [Aurora Viewer](https://aurora-viewer.org) or
  [Firestorm](https://www.firestormviewer.org/) (OpenSim build)

## Build

```sh
git clone https://github.com/FlintWave/welkin.git
cd welkin
cargo build --workspace --release
```

This produces two binaries in `target/release/`:

- `welkin-robust` runs the grid services (login, inventory, assets, grid registry)
- `welkin-region` runs the region simulator (scene, physics, scripting, viewer connections)

## Configure

Welkin uses TOML configuration files. A minimal setup needs two:

**robust.toml** (grid services):

```toml
[DatabaseService]
ConnectionString = "sqlite:robust.db?mode=rwc"
```

**welkin.toml** (region simulator):

```toml
[Region]
RegionName = "Test Region"
RegionLocX = 1000
RegionLocY = 1000

[Network]
ExternalHostName = "127.0.0.1"
InternalPort = 9000

[DatabaseService]
RegionConnectionString = "sqlite:region.db?mode=rwc"
RobustConnectionString = "sqlite:robust.db?mode=rwc"

[GridService]
GridServerURI = "http://127.0.0.1:8003"

[Scene]
PhysicsEnabled = true
ScriptsEnabled = true
```

Any setting can be overridden with an environment variable using the pattern
`OPENSIM__Section__Key=value`. For example,
`OPENSIM__Region__RegionName=MyRegion` overrides the region name without
editing the file.

If you are migrating from a C# OpenSimulator installation, `welkin-migration`
can import your existing `.ini` configuration files.

See the [configuration reference](configuration.md) for all
available sections and keys.

## Run

Start the grid services first, then the region simulator:

```sh
# Terminal 1: grid services (public port 8002, private port 8003)
./target/release/welkin-robust --config robust.toml

# Terminal 2: region simulator (UDP 9000)
./target/release/welkin-region --config welkin.toml
```

On first run the web portal at `http://localhost:9000/` walks you through
database setup, creating the first user account, and placing the region on
the grid map.

## Connect a viewer

Point your viewer at the login URI `http://127.0.0.1:8002/`:

- Aurora Viewer: launch with `--loginuri http://127.0.0.1:8002/`
- Firestorm: open Grid Manager, add a new grid with the login URI above,
  then select it and log in

Log in with the user account you created during setup.

## Standalone vs external grid

The steps above run a standalone grid: both binaries on one machine with a
local database. To connect a region to an existing grid instead, change
`GridServerURI` in welkin.toml to point at that grid's private service
endpoint. The region registers itself with the remote ROBUST and appears
on the grid map.

For OSGrid, set `GridServerURI` to OSGrid's grid service URL and follow
their registration process. See [OSGrid compatibility](osgrid-compatibility.md)
for details.
