<!-- SPDX-License-Identifier: LGPL-3.0-or-later -->
# Configuration reference

[← Back to docs](README.md) · [Quick start](quick-start.md) · [Architecture](architecture.md)

Welkin reads TOML configuration files at startup. Each binary takes a
`--config` flag pointing to its config file; defaults are `welkin.toml`
for the region simulator and `robust.toml` for grid services.

## File format

Standard TOML. Top-level tables become config sections; keys inside them
are typed as string, integer, boolean, or float.

```toml
[Region]
RegionName = "My Region"
RegionLocX = 1000
RegionLocY = 1000

[Scene]
PhysicsEnabled = true
ScriptsEnabled = true
```

Values from one section can reference another with `${Section|Key}`
substitutions. The config loader resolves these iteratively (up to 10
passes), so forward references work.

```toml
[Network]
ExternalHostName = "grid.example.com"

[GridService]
GridServerURI = "http://${Network|ExternalHostName}:8003"
```

## Environment overrides

Any config value can be overridden by an environment variable prefixed
with `OPENSIM__`, using double underscores to separate section and key:

```sh
export OPENSIM__Region__RegionName="Production Region"
export OPENSIM__Network__InternalPort=9010
```

Values are auto-typed: `true`/`false` become booleans, numeric strings
become integers or floats, everything else is a string. Environment
overrides apply after the config file loads and before substitution
resolution, so they participate in `${Section|Key}` expansion.

## OpenSimulator INI import

The `welkin-config` crate can read C# OpenSimulator `.ini` files via
`import_ini()`. This lets the migration tool (`welkin-migration`) convert
an existing `OpenSim.ini` or `Robust.ini` to TOML without manual
rewriting. Keys are lowercased during import (matching the `configparser`
crate's behavior).

## welkin-region (region simulator)

CLI: `welkin-region --config <path> [--region-config <path>]`

Defaults: `--config welkin.toml`. The optional `--region-config` flag
loads a second file that merges on top, for per-region overrides in
multi-region setups.

| Section | Key | Type | Default | Purpose |
|---|---|---|---|---|
| Region | RegionName | string | My Region | Display name shown to viewers |
| Region | RegionLocX | int | 1000 | Grid X coordinate |
| Region | RegionLocY | int | 1000 | Grid Y coordinate |
| Network | ExternalHostName | string | 127.0.0.1 | Public hostname or IP that viewers connect to |
| Network | InternalPort | int | 9000 | UDP port for LLUDP viewer traffic |
| DatabaseService | RegionConnectionString | string | sqlite:region.db | Database URL for region object storage |
| DatabaseService | RobustConnectionString | string | sqlite:robust.db | Database URL for grid service queries |
| GridService | GridServerURI | string | (none) | URL of the ROBUST grid service private port |
| Scene | PhysicsEnabled | bool | true | Run Rapier physics simulation |
| Scene | ScriptsEnabled | bool | true | Run LSL/OSSL script execution |
| Estate | EstateOwner | string | (none) | Avatar UUID of the estate owner. Only the owner and managers may terraform or use estate tools |
| Estate | EstateManagers | string | (none) | Comma-separated avatar UUIDs with estate manager rights |
| Permissions | serverside_object_permissions | bool | true | Set false to switch all permission checks off (development only) |
| Permissions | region_owner_is_god | bool | true | Treat the estate owner as an administrator |

Permission checks are on by default. Without `EstateOwner` nobody has estate
rights; avatars can still edit, move and delete objects they own. The region
logs a warning at start-up when no owner is configured.

## welkin-robust (grid services)

CLI: `welkin-robust --config <path> [--public-port <port>] [--private-port <port>]`

Defaults: `--config robust.toml`, `--public-port 8002`, `--private-port 8003`.
The public port handles viewer login (XMLRPC) and user-facing caps. The
private port handles inter-service traffic (grid registration, asset
storage, inventory queries).

| Section | Key | Type | Default | Purpose |
|---|---|---|---|---|
| DatabaseService | ConnectionString | string | sqlite:robust.db | Database URL for all grid services |
| DatabaseService | RobustConnectionString | string | (from ConnectionString) | Override when grid services need a separate database |

## Database URLs

All database URLs use the same format across both binaries:

| Backend | URL format | Notes |
|---|---|---|
| SQLite | `sqlite:path.db?mode=rwc` | `?mode=rwc` creates the file if absent |
| MariaDB/MySQL | `mysql://user:pass@host/db` | Via sqlx with the MySQL driver |
| PostgreSQL | `postgres://user:pass@host/db` | Via sqlx with the Postgres driver |

SQLite is the default for local development and single-server grids.
MariaDB or PostgreSQL is recommended for production grids with multiple
regions.

## Logging

The region simulator reads a `[Logging]` section at startup. Welkin does
not read OpenSimulator's log4net XML file, so the `logconfig` key under
`[Startup]` has no effect. Every key below is optional.

| Key | Type | Default | Purpose |
|---|---|---|---|
| Directory | string | logs | Folder for log files; created if absent |
| FilePrefix | string | region.log | Start of each log file name; the rotation date is appended |
| Rotation | string | daily | `hourly`, `daily`, or `never`; any other value means `daily` |
| MaxFiles | int | 14 | Rotated files to keep; `0` keeps all of them |
| Level | string | info | Base level: `error`, `warn`, `info`, `debug`, or `trace` |
| Filter | string | (empty) | Extra per-module directives, appended to `Level` |
| Json | bool | false | Write JSON lines instead of plain text, to the console and the file |
| Console | bool | true | Write logs to standard error |
| File | bool | true | Write logs to rotating files |

`Filter` takes comma-separated `target=level` pairs. With `Level = "info"`
and `Filter = "welkin_net=debug,welkin_scene=warn"`, Welkin logs at info
level, with debug output from the network crate and only warnings from the
scene crate.

```toml
[Logging]
Directory = "logs"
Rotation = "hourly"
MaxFiles = 48
Level = "info"
Filter = "welkin_net=debug"
```

If the `RUST_LOG` environment variable is set, it replaces both `Level`
and `Filter`. An invalid `RUST_LOG` or `Filter` value falls back to `info`.
`MaxFiles` and `Rotation` apply only when `File` is true. File output goes
through a background writer, so lines written just before a crash can be
lost.

## Backup

The region simulator saves changed objects to the database on a timer.

| Key | Type | Default | Purpose |
|---|---|---|---|
| IntervalSeconds | int | 900 | Seconds between saves; values below 5 are raised to 5, and values of 0 or less use the default |

```toml
[Backup]
IntervalSeconds = 300
```

Each save writes only objects that changed since the last one, and logs
the count at info level when it is above zero.

OpenSimulator saves on a frame count instead: the scene writes to storage
every `UpdateStorageEveryNFrames` frames, 200 by default. Welkin uses
wall-clock time on purpose, so the save interval does not shift with the
frame rate. If you are migrating, set `IntervalSeconds` for the delay you
want and drop `UpdateStorageEveryNFrames`; Welkin ignores that key.

## Module configuration

Region modules read their settings through the `ModuleConfig` trait,
which wraps the same section/key lookup with typed defaults:

- `get_string(section, key, default) -> String`
- `get_int(section, key, default) -> i64`
- `get_bool(section, key, default) -> bool`
- `get_float(section, key, default) -> f64`

Each module documents its own config section. Some examples:

```toml
[AssetCache]
MemoryCacheEnabled = true
MemoryCacheMaxItems = 30000
MemoryCacheTTL = 120
FileCacheEnabled = true
CacheDirectory = "assetcache"

[Network]
IPBanList = "1.2.3.4, 10.0.0."

[LandService]
ServiceURL = "http://127.0.0.1:8003"
Timeout = 25000
```

Network entries ending with a dot (like `10.0.0.`) are treated as
subnet prefixes by the IP ban module.

## Config merge order

When multiple sources provide the same key, later sources win:

1. Base config file (`--config`)
2. Region config file (`--region-config`, region binary only)
3. Environment variables (`OPENSIM__*`)
4. Substitution resolution (`${Section|Key}`)
