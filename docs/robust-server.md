<!-- SPDX-License-Identifier: LGPL-3.0-or-later -->
# ROBUST grid services server

[← Back to docs](README.md) · [Operator guide](operator-guide.md) · [Integration architecture](integration-architecture.md) · [Container deployment](container-guide.md)

ROBUST is the grid services process. It handles login, asset storage,
inventory, user accounts, presence, grid registration and every other
service that regions and viewers share. The name comes from C# OpenSimulator,
where the same process serves the same role. Welkin's `welkin-robust` binary
is a wire-compatible replacement.

## What it does

ROBUST listens on two ports. The public port (default 8002) faces viewers
and serves XML-RPC login, LLSD login, grid info and map tiles. The private
port (default 8003) faces regions and serves all service-to-service traffic:
asset fetch/store, inventory queries, grid registration, presence updates,
friends, groups, profiles, estates, auth, agent preferences, mute lists,
offline IM, map image upload and authorization.

The split keeps inter-service endpoints off the public network. In
production, only port 8002 needs to be reachable by viewers. Port 8003
should be restricted to the region hosts.

## Services hosted

| Service | Port | Protocol | Role |
|---|---|---|---|
| Login | public | XML-RPC, LLSD | Viewer authentication, session creation |
| Grid info | public | JSON (GET) | Grid name, login URI for viewer discovery |
| Map tiles | public | HTTP GET | Map tile images for the viewer world map |
| Asset | private | LLSD | Store and retrieve assets (textures, meshes, scripts) |
| Inventory | private | LLSD | Folder tree, item CRUD, library folders |
| Grid | private | LLSD | Region registration, lookup, neighbour queries |
| Presence | private | LLSD | Who is online and where |
| GridUser | private | LLSD | Last-seen region, home location |
| Accounts | private | LLSD | User account CRUD, search |
| Auth | private | LLSD | Password verification, token management |
| Authorization | private | LLSD | Access control checks |
| Avatar | private | LLSD | Baked appearance data |
| Friends | private | LLSD | Friend list, online notifications |
| Agent preferences | private | LLSD | Per-user settings |
| Mute | private | LLSD | Mute list storage |
| Offline IM | private | LLSD | Store and deliver offline messages |
| Groups | private | LLSD | Group membership, notices, roles |
| Estate | private | LLSD | Estate settings, access lists |
| Profiles | private | LLSD | User profiles, picks, classifieds |

## Configuration

ROBUST reads `robust.toml` by default (override with `--config <path>`).
The minimal configuration sets just the database connection:

```toml
[DatabaseService]
ConnectionString = "Server=localhost;Port=3306;Database=opensim;User=opensim;Password=secret"
```

For SQLite (development only):

```toml
[DatabaseService]
ConnectionString = "sqlite:robust.db?mode=rwc"
```

Environment variables override any TOML setting. The format is
`OPENSIM__Section__Key=value`:

```sh
OPENSIM__DatabaseService__ConnectionString="Server=db;Port=3306;Database=opensim;User=opensim;Password=secret"
```

See the [operator guide](operator-guide.md) for the full configuration
reference, including database setup for MariaDB, PostgreSQL and SQLite.

## Running

### Command line

```sh
welkin-robust --config robust.toml --public-port 8002 --private-port 8003
```

All flags have defaults, so a bare `welkin-robust` works for local
development if `robust.toml` exists in the working directory.

| Flag | Default | Purpose |
|---|---|---|
| `--config` | `robust.toml` | Path to the TOML configuration file |
| `--public-port` | `8002` | TCP port for login, grid info and map tiles |
| `--private-port` | `8003` | TCP port for service-to-service traffic |

### With a region

Start ROBUST first, then the region simulator:

```sh
# Terminal 1: grid services
welkin-robust

# Terminal 2: region simulator
welkin-region --config welkin.toml
```

The region connects to ROBUST's private port to register itself, fetch
inventory and assets, and report presence changes. The viewer connects
to ROBUST's public port to log in, then to the region's UDP and HTTP
ports for in-world traffic.

### With Podman Compose

The repository's `compose.prod.yaml` starts the full stack (database,
ROBUST, region, portal). See the [container guide](container-guide.md)
for details.

### With systemd

The installer creates a `welkin-robust.service` unit:

```sh
sudo systemctl enable --now welkin-robust
journalctl -u welkin-robust -f
```

## Health and monitoring

ROBUST exposes `/health` (returns `ok`, HTTP 200) and `/metrics`
(Prometheus text format) on both ports. Use `/health` for load balancer
probes and container orchestration readiness checks.

Key metrics to watch:

- `opensim_caps_requests_total`: HTTP request throughput across services
- Login latency and error rate (from the XML-RPC login handler)
- Database connection pool usage (from `sqlx` metrics)

## Database

ROBUST uses `welkin-data` to access 13 typed stores over `sqlx`. It
supports MariaDB 10.5+, PostgreSQL 14+ and SQLite. On first startup the
migration runner applies 53 embedded SQL files matching the C# OpenSimulator
schema. An existing C# database opens without modification.

Run `osim-migrate validate-db` before connecting to a database that was
created by C# OpenSimulator, to confirm schema compatibility.

## Security notes

The private port serves all inter-service endpoints without authentication.
These endpoints trust that callers are authorized region processes. In
production, restrict port 8003 to region hosts using firewall rules or
network segmentation. Do not expose it to the public internet.

The public port serves login (which accepts passwords) and grid info
(read-only). Place a reverse proxy with TLS in front of port 8002 for
production use. ROBUST itself serves plain HTTP.

See the [operator guide](operator-guide.md) for TLS setup, firewall
recommendations and rate limiting.

## Relationship to C# OpenSimulator

Welkin's ROBUST speaks the same wire protocol as C# OpenSimulator's
`Robust.exe`. A Welkin region can connect to a C# ROBUST, and a C#
region can connect to a Welkin ROBUST. This makes incremental migration
possible: replace one component at a time while the rest of the grid
keeps running.

See the [migration guide](migration-guide.md) for the full migration
process.
