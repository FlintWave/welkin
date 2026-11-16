# Container Deployment Guide

<!-- SPDX-License-Identifier: LGPL-3.0-or-later -->

Production container guide for Welkin (Robust, Region, Migrate).

> [!NOTE]
> The web portal is a scaffold and is not part of any running service. There is
> no portal container, no portal port, and no portal health endpoint. Earlier
> versions of this guide listed one; those references are removed.

## Building Images

The multi-stage `Containerfile` compiles all binaries in a single builder stage
and produces a minimal distroless runtime image.

```bash
# Full image (robust, region, osim-migrate)
podman build -f Containerfile -t welkin:latest .

# Region-only image (smaller, single binary)
podman build -f Containerfile.region -t welkin-region:latest .
```

Builds use `cargo-chef` for dependency caching. The first build downloads and
compiles all crates; subsequent builds reuse cached layers unless `Cargo.toml`
or `Cargo.lock` change.

## Running with Podman Compose

```bash
# 1. Create your environment file from the template
cp .env.example .env
# 2. Edit .env -- change every "change-me-*" value
vi .env
# 3. Start the production stack
podman compose -f compose.prod.yaml up -d
# 4. Check service health
podman compose -f compose.prod.yaml ps
```

> [!WARNING]
> The stack uses MySQL, and Welkin has only been run on SQLite. Expect failures
> until the MariaDB and PostgreSQL issues in the
> [configuration reference](configuration.md#database-urls) are fixed.

The production stack starts: MySQL 8.0, Robust (grid services), Region
(simulator), and WolfVoice (voice sidecar).

## Configuration

All runtime configuration is driven by environment variables in `.env`:

| Variable                | Purpose                          | Default              |
|-------------------------|----------------------------------|----------------------|
| `MYSQL_ROOT_PASSWORD`   | Database root password           | *(must set)*         |
| `MYSQL_PASSWORD`        | App database password            | *(must set)*         |
| `OPENSIM_DB_CONNECTION` | Connection string for services   | *(must set)*         |
| `RUST_LOG`              | Log level filter                 | `info`               |
| `OPENSIM_LOG_JSON`      | JSON-structured log output       | `false`              |
| `ROBUST_HTTP_PORT`      | Robust listener port             | `8002`               |
| `REGION_HTTP_PORT`      | Region listener port             | `9000`               |
| `WOLFVOICE_SECRET_KEY`  | Voice sidecar shared secret      | *(must set)*         |

See `.env.example` for the full list with inline documentation.

## Scaling Regions

Each region instance needs a unique UDP port for LLUDP viewer traffic:

```bash
# Start three region instances
podman compose -f compose.prod.yaml up -d --scale region=3
```

When scaling beyond one region, you must assign distinct external UDP ports.
Override with per-instance port mappings or use host networking. Each region
also needs its own region configuration (location, name, UUID) passed via
environment or a mounted config volume.

## Backup Procedures

### Database Backups

```bash
# Dump the entire opensim database
podman compose -f compose.prod.yaml exec db \
  mysqldump -u opensim -p"$MYSQL_PASSWORD" opensim > backup-$(date +%F).sql
```

### Volume Backups

```bash
# Back up the persistent database volume
podman run --rm -v welkin_db-data:/data -v $(pwd):/backup \
  alpine tar czf /backup/db-data-$(date +%F).tar.gz -C /data .

# Back up region data
podman run --rm -v welkin_region-data:/data -v $(pwd):/backup \
  alpine tar czf /backup/region-data-$(date +%F).tar.gz -C /data .
```

### OAR Backups

OAR (OpenSim Archive) saves are triggered through the region's admin API or
console. The `oar-backups` volume is mounted at `/data/oar` inside the region
container for storing exports.

## Monitoring

Robust and Region expose Prometheus metrics and a health
endpoint on its HTTP port:

| Endpoint   | Response                      |
|------------|-------------------------------|
| `/health`  | `ok` (HTTP 200 when healthy)  |
| `/metrics` | Prometheus text format        |

Configure Prometheus scrape targets:

```yaml
scrape_configs:
  - job_name: welkin-robust
    static_configs:
      - targets: ['robust:8002']
  - job_name: welkin-region
    static_configs:
      - targets: ['region:9000']
```

## Troubleshooting

### Checking Logs

```bash
# All services
podman compose -f compose.prod.yaml logs -f

# Single service, last 100 lines
podman compose -f compose.prod.yaml logs --tail=100 robust

# Enable debug logging for a single crate
# Set in .env: RUST_LOG=welkin_robust=debug,info
podman compose -f compose.prod.yaml up -d robust
```

### Health Checks

```bash
# From the host (adjust port if changed in .env)
curl http://localhost:8002/health   # Robust
curl http://localhost:9000/health   # Region
```

### Common Issues

- **Database connection refused**: the `db` service may still be starting.
  Robust and Region wait for the database health check, but if the check
  times out, restart the dependent services.
- **Port conflicts**: ensure no other process binds 8002, 9000, or 3306
  on the host. Remap in `.env` if needed.
- **Region not connecting to grid**: verify `REGION_GRID_URI` points to the
  Robust service name (`http://robust:8002`) on the Podman network.
- **Graceful shutdown hangs**: the runtime handles `SIGTERM` via tokio; the
  default stop timeout is 10 seconds. Increase with
  `podman compose stop -t 30` if regions need longer to persist state.
