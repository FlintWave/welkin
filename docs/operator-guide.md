<!-- SPDX-License-Identifier: LGPL-3.0-or-later -->

# Operator guide

[<- Back to docs](index.md) | [Architecture](architecture.md) | [Test grid deployment](test-grid-deployment.md) | [Migration guide](migration-guide.md)

Complete reference for installing, configuring, running, and maintaining an
OpenSimulator grid using the Rust implementation. For a quick test deployment,
see the [test grid deployment guide](test-grid-deployment.md) instead.

---

## Installation

### From source

Requirements: Rust 1.87+, a C toolchain (for native dependencies: openjpeg-sys,
Luau), cmake, and pkg-config.

```sh
git clone <repo-url> welkin
cd welkin
cargo build --release
```

Binaries are written to `target/release/`:

| Binary | Purpose |
|---|---|
| `welkin-robust` | Grid services (login, asset, inventory, grid, presence, etc.) |
| `welkin-region` | Region simulator |
| `opensim-migrate` | Migration toolkit (DB validator, password upgrade, config converter) |
| `opensim-conform` | Conformance testing and load testing |

### From Docker / Podman image

The multi-stage `Containerfile` produces static musl-linked binaries in an
Alpine-based runtime image:

```sh
podman build -t welkin .
```

The image contains `welkin-robust` and `welkin-region` in `/usr/local/bin/`.
It runs as a non-root `opensim` user.

### From binary release

Download the release archive for your platform and extract it:

```sh
tar xzf welkin-x86_64-linux.tar.gz
cd welkin
./install/install-archive.sh
```

The archive installer places binaries, creates a systemd unit
(`welkin-robust.service`), and writes a starter configuration.

### One-command installer

For Debian/Ubuntu and Fedora hosts with systemd:

```sh
curl -sSf https://raw.githubusercontent.com/.../install/install.sh | bash
```

The installer detects the distribution, sets up the database, writes
configuration, and creates the systemd unit. The first-run wizard is part of the portal
scaffold and does not run.
`install/upgrade.sh` and `install/uninstall.sh` handle upgrades (preserving
data) and removal.

---

## Configuration

Configuration uses TOML files with environment variable overrides. The
priority chain (highest wins):

1. Environment variables (`OPENSIM__Section__Key=value`)
2. Explicit config file (TOML)
3. Grid profile defaults

### Environment variable format

Any setting can be overridden by an environment variable named
`OPENSIM__<Section>__<Key>`. For example:

```sh
OPENSIM__Database__ConnectionString="Server=db;Port=3306;Database=opensim;User=opensim;Password=secret"
OPENSIM__Network__http_listener_port="8003"
OPENSIM__Voice__WolfvoiceURI="http://wolfvoice:7880"
```

### Grid profiles

The `[GridProfile]` section selects a deployment mode:

| Profile | Description |
|---|---|
| `standalone` | All services inside the region process on localhost:9000 |
| `own-grid` | Separate Robust process; service URLs default to localhost:8003 |
| Custom (e.g. `osgrid`) | External grid; loads `profiles/<name>.toml` |

Set the profile in the config file or via environment:

```sh
OPENSIM__GridProfile__Name="own-grid"
```

Custom profiles are TOML files in the `profiles/` directory. See
`welkin-config/src/profile.rs` for the format. The
`osim-migrate convert-config` command can generate a custom profile from a
C# `GridCommon.ini`.

### Configuration reference

#### `[Network]`

| Key | Default | Description |
|---|---|---|
| `http_listener_port` | `9000` (region) / `8003` (robust) | TCP port for HTTP services |
| `ExternalHostName` | `SYSTEMIP` | Public hostname or IP for viewer connections |

#### `[Database]`

| Key | Default | Description |
|---|---|---|
| `ConnectionString` | (required) | Database connection string (see Database setup below) |
| `StorageProvider` | auto-detected | `mysql`, `postgres`, or `sqlite` |

#### `[GridService]`

| Key | Default | Description |
|---|---|---|
| `GridServerURI` | from profile | URL of the grid service |
| `AllowDuplicateNames` | `false` | Allow regions with duplicate names |

#### `[AssetService]`

| Key | Default | Description |
|---|---|---|
| `AssetServerURI` | from profile | URL of the asset service |

#### `[Physics]`

| Key | Default | Description |
|---|---|---|
| `Engine` | `rapier` | Physics backend (`rapier` or `null`) |
| `PhysicsTickRate` | `45.0` | Simulation steps per second |
| `MaxPrimsPerFrame` | `200` | Object updates per physics step |

#### `[Scripts]`

| Key | Default | Description |
|---|---|---|
| `Enabled` | `true` | Enable/disable script execution |
| `MemoryLimitBytes` | `65536` | Per-script Luau memory limit |
| `MaxEventQueueSize` | `300` | Maximum pending events per script |
| `OsslThreatLevel` | `VeryLow` | Default OSSL threat ceiling (`None`, `VeryLow`, `Low`, `Moderate`, `High`, `VeryHigh`, `Severe`) |

#### `[Voice]`

| Key | Default | Description |
|---|---|---|
| `Enabled` | `false` | Enable WebRTC voice |
| `WolfvoiceURI` | `http://localhost:7880` | wolfvoice sidecar endpoint |

#### `[Hypergrid]`

| Key | Default | Description |
|---|---|---|
| `Enabled` | `false` | Enable hypergrid teleports |
| `GatekeeperURI` | derived from `ExternalHostName` | Public gatekeeper endpoint |

#### `[Logging]`

Logging is controlled by the `RUST_LOG` environment variable using the
`tracing` / `EnvFilter` syntax:

```sh
RUST_LOG=info                        # default
RUST_LOG=debug                       # verbose
RUST_LOG=welkin_net=trace,info      # trace LLUDP, info elsewhere
RUST_LOG=welkin_services=debug      # debug grid service handlers
```

Set `OPENSIM__Logging__json=true` to switch to structured JSON log output.

---

## Database setup

Three backends are supported via `sqlx`. All use the same schema (the 53 C#
migration files are embedded and applied automatically on startup).

> [!WARNING]
> Only SQLite has been run and verified. MariaDB and PostgreSQL are supported in
> the code but untested, with known problems; see the
> [configuration reference](configuration.md#database-urls).

### MariaDB / MySQL (intended for production, not yet verified)

```sh
# Create the database and user
mysql -u root -p <<'SQL'
CREATE DATABASE opensim CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
CREATE USER 'opensim'@'%' IDENTIFIED BY '<password>';
GRANT ALL PRIVILEGES ON opensim.* TO 'opensim'@'%';
FLUSH PRIVILEGES;
SQL
```

Connection string:

```
Server=localhost;Port=3306;Database=opensim;User=opensim;Password=<password>
```

MariaDB 10.5+ or MySQL 8.0+ is required.

### PostgreSQL

```sh
createuser -P opensim
createdb -O opensim opensim
```

Connection string:

```
Host=localhost;Port=5432;Database=opensim;Username=opensim;Password=<password>
```

PostgreSQL 14+ is recommended.

### SQLite (development only)

Connection string:

```
Data Source=/path/to/opensim.db
```

SQLite is single-writer and not suitable for production grids with concurrent
regions.

### Existing C# databases

A database created by C# OpenSimulator 0.9.x opens without modification. Run
`osim-migrate validate-db` to check schema compatibility before connecting:

```sh
osim-migrate validate-db --connection "Server=localhost;Port=3306;Database=opensim;User=opensim;Password=<password>"
```

---

## Running services

### Standalone mode

All services run in one process:

```sh
OPENSIM__GridProfile__Name=standalone \
OPENSIM__Database__ConnectionString="<connection>" \
  welkin-region
```

### Grid mode (Robust + Region)

Run Robust (grid services) and the region simulator as separate processes:

```sh
# Terminal 1: grid services
OPENSIM__Database__ConnectionString="<connection>" \
OPENSIM__Network__http_listener_port=8003 \
  welkin-robust

# Terminal 2: region simulator
OPENSIM__GridProfile__Name=own-grid \
OPENSIM__Database__ConnectionString="<connection>" \
OPENSIM__Network__http_listener_port=9000 \
OPENSIM__GridService__GridServerURI=http://localhost:8003 \
  welkin-region
```

### Multi-region

A single `welkin-region` process can host multiple regions. Each region needs
a unique UDP port for LLUDP traffic. Configure additional regions through
configuration files.

### Docker Compose

The `compose.yaml` in the repository root runs a complete grid (MariaDB +
Robust + Region). See the [test grid deployment](test-grid-deployment.md)
guide.

### systemd

The installer creates `welkin-robust.service`:

```sh
sudo systemctl enable --now welkin-robust
sudo systemctl status welkin-robust
journalctl -u welkin-robust -f
```

---

## Backup and restore

### OAR (region content)

Save a region's terrain, objects, and scripts to an OAR archive:

```sh
welkin-region save-oar /backups/MyRegion-$(date +%Y%m%d).oar
```

Restore from an OAR:

```sh
welkin-region load-oar /backups/MyRegion-20261114.oar
```

Multi-region OARs are supported. Missing regions are created automatically
during load.

### IAR (inventory)

Export a user's inventory to an IAR archive:

```sh
welkin-region save-iar "FirstName" "LastName" "/*" /backups/inventory.iar
```

Import:

```sh
welkin-region load-iar "FirstName" "LastName" "/" /backups/inventory.iar
```

### Database dumps

For full-grid backup, dump the database directly:

```sh
# MariaDB/MySQL
mysqldump -u opensim -p opensim > opensim-backup-$(date +%Y%m%d).sql

# PostgreSQL
pg_dump -U opensim opensim > opensim-backup-$(date +%Y%m%d).sql

# SQLite
sqlite3 opensim.db ".backup opensim-backup-$(date +%Y%m%d).db"
```

> [!TIP]
> Use OAR for content that you need to load into a different grid or move
> between regions. Use database dumps for disaster recovery of the entire
> grid state.

---

## Monitoring

### Prometheus metrics

Both binaries expose a `/metrics` endpoint in Prometheus exposition format.
Scrape `http://<host>:<port>/metrics` from your Prometheus instance.

Key metrics:

| Metric | Type | Description |
|---|---|---|
| `opensim_region_agents_total` | gauge | Connected avatars |
| `opensim_region_prims_total` | gauge | Scene objects |
| `opensim_physics_step_duration_seconds` | histogram | Physics tick time |
| `opensim_lludp_packets_sent_total` | counter | Outbound LLUDP packets |
| `opensim_lludp_packets_received_total` | counter | Inbound LLUDP packets |
| `opensim_script_events_processed_total` | counter | Script event throughput |
| `opensim_caps_requests_total` | counter | Capability HTTP requests |

### Health endpoint

`GET /health` returns `ok` (HTTP 200) when the service is running. Use it for
load balancer health checks and container orchestration.

### Alerting recommendations

- Physics tick > 15ms sustained: region is overloaded
- Agent count approaching 40 per region: performance may degrade
- LLUDP packet loss ratio > 5%: network issue
- Script event backlog growing: scripts are starved

---

## Log management

### RUST_LOG levels

| Level | Use case |
|---|---|
| `error` | Production minimum |
| `warn` | Recommended production default |
| `info` | Normal operation (default) |
| `debug` | Development and troubleshooting |
| `trace` | Protocol-level debugging (very verbose) |

### Structured logging

Enable JSON output for log aggregation (ELK, Loki, etc.):

```sh
OPENSIM__Logging__json=true
```

### Log rotation

When running under systemd, logs go to the journal. Configure rotation in
`/etc/systemd/journald.conf`. When running directly, pipe through a log
rotator or redirect to a file with `logrotate`.

---

## Troubleshooting

### Port conflicts

**Symptom:** `Address already in use` on startup.

Check what is listening:

```sh
ss -tlnp | grep -E '(8003|9000)'
```

Either stop the conflicting process or change the port in configuration.

### Database connection failures

**Symptom:** `failed to connect to database` or `Connection refused`.

- Verify the database is running: `systemctl status mariadb`
- Verify the connection string matches the database credentials
- Check that the database user has permissions on the target database
- For containers, ensure the database service is healthy before starting Robust

### Viewer cannot connect

**Symptom:** Aurora Viewer shows "Login failed" or hangs at "Connecting..."

- Verify Robust is running and `/health` returns `ok`
- Verify the login URI in the viewer matches the Robust port (default 8003)
- Check firewall: TCP 8003 (login/services) and TCP+UDP 9000 (region) must
  be open
- Check `ExternalHostName` is set to the reachable IP or hostname, not
  `SYSTEMIP` (which resolves to the machine's primary IP)

### Physics issues

**Symptom:** Objects fall through terrain, avatars stuck, collisions missing.

- Check `opensim_physics_step_duration_seconds` -- if consistently > 11ms the
  physics engine cannot keep up at 45 FPS
- Reduce `MaxPrimsPerFrame` or the number of physical objects
- Verify the physics engine is `rapier` (not `null`)

### Script errors

**Symptom:** Script does not run, `Script error` in chat.

- Check `RUST_LOG=welkin_script_vm=debug` for detailed error output
- Verify `[Scripts] Enabled = true`
- Check the OSSL threat level if the script uses `os*` functions
- Memory limit: increase `MemoryLimitBytes` if scripts hit the ceiling

---

## Performance tuning

### Thread pool sizing

The async runtime uses Tokio with its default thread pool (one worker per
CPU core). Override with:

```sh
TOKIO_WORKER_THREADS=4
```

### Physics tick rate

The default 45 FPS matches C# OpenSimulator. For regions with heavy physics
loads, lower it:

```
[Physics]
PhysicsTickRate = 22.5
```

### Script limits

Constrain script resource usage per region:

```
[Scripts]
MemoryLimitBytes = 65536       # per-script Luau heap limit
MaxEventQueueSize = 300        # pending events before dropping
```

### Asset caching

When running in grid mode, the region caches assets locally to reduce
requests to Robust:

```
[AssetService]
CacheEnabled = true
CacheDirectory = "/var/cache/opensim/assets"
CacheExpirySeconds = 86400
```

---

## Security

### TLS

The binaries serve plain HTTP. For TLS, place a reverse proxy in front:

```nginx
# nginx example
server {
    listen 443 ssl;
    server_name grid.example.com;

    ssl_certificate     /etc/letsencrypt/live/grid.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/grid.example.com/privkey.pem;

    location / {
        proxy_pass http://127.0.0.1:8003;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

LLUDP (UDP 9000) is not proxied; it must be reachable directly.

### Authentication

- Planned: portal staff passwords use Argon2id (decision G2); the portal is a
  scaffold and not deployed
- Grid user passwords are MD5 by default (C# compatibility); upgrade to
  Argon2id with `osim-migrate upgrade-passwords`
- OSSL function access is gated by threat level with per-function overrides

### Account creation

The grid's `createuser` method is off by default (`[UserAccountService]
AllowCreateUser = false`). Turn it on only while you create accounts, then set
it back to false and restart `welkin-robust`. While it is on, anyone who can
reach the private service port can create accounts. The repository's
development `robust.toml` and compose file enable it for local testing; do not
copy that into a production grid. See the
[configuration reference](configuration.md#welkin-robust-grid-services).

### Estate owner and permissions

Permission checks are on by default (`[Permissions]
serverside_object_permissions`). Set `[Estate] EstateOwner` to your avatar UUID
and add managers with `EstateManagers`; without an owner, nobody can terraform
or use the estate tools, though avatars can still edit objects they own. Turn
the permission checks off only on a development region: with them off, any
avatar can edit or delete any object. Each avatar's terrain edits are also
limited to one every 20 ms. See the
[configuration reference](configuration.md#welkin-region-region-simulator).

### Private service port

`welkin-robust` binds its private port (8003 by default) to `127.0.0.1`. That
port serves account and authentication endpoints with no authentication, so
treat any wider bind as a decision. Use `--private-bind` or
`WELKIN_PRIVATE_BIND` only to reach it from a region host or container network,
and restrict it by firewall to those hosts. See the
[configuration reference](configuration.md#welkin-robust-grid-services).

### Rate limiting

Apply rate limiting at the reverse proxy layer. Key endpoints to protect:

- `/login` (XML-RPC login)
- `/assets` (asset upload/download)

### Firewall recommendations

| Port | Protocol | Expose to |
|---|---|---|
| 8003 | TCP | Viewers and regions (login and grid services) |
| 9000 | TCP+UDP | Viewers (region traffic) |
| 3306/5432 | TCP | Internal only (database) |
| 7880 | TCP | Internal only (wolfvoice sidecar) |

> [!WARNING]
> Admin-style service endpoints are not gated in code (decision S5). They
> must be restricted by network placement or a reverse proxy. Do not expose
> Robust directly to the public internet without an access control layer.

---

## What survives a restart

A region saves its objects, terrain and estate settings to its database, and
loads them again at start. Objects and terrain are written on the backup timer
(`[Backup] IntervalSeconds`, 900 by default), so a crash can lose edits made
since the last save. Avatars' worn attachments are remembered in the grid's
avatar service and worn again at the next login. Scripts are saved and restored
with their objects; a saved script whose object no longer exists is skipped and
deleted at start.

All of this is keyed by the region UUID. Leave `[Region] RegionUUID` unset to let
the region generate one on its first start and keep it, or set it yourself. Do
not rename a region or move it to another location without setting `RegionUUID`
to the old value; the region would get a new UUID and would not find its data.
See the [configuration reference](configuration.md#welkin-region-region-simulator).

## Upgrading

### Version compatibility

The Rust implementation maintains wire and schema compatibility with C#
OpenSimulator 0.9.x (decision R2). Database schema is stable; the 53
embedded migration files match the C# schema.

### Upgrade steps

1. Back up the database (see Backup and restore above)
2. Stop the running services
3. Replace the binaries (rebuild from source, pull new image, or extract new
   release archive)
4. Run `osim-migrate validate-db` to confirm schema compatibility
5. Start the services
6. Verify with the [test grid checklist](test-grid-deployment.md#verifying-the-deployment)

### Rolling upgrade (grid mode)

In grid mode, upgrade Robust first, then regions one at a time. Since regions
are stateless with respect to the grid services, a region restart does not
affect other regions.

See the [migration guide](migration-guide.md) for migrating from C#
OpenSimulator to the Rust implementation.
