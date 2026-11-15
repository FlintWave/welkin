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
