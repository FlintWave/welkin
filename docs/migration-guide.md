<!-- SPDX-License-Identifier: LGPL-3.0-or-later -->

# Migration guide: C# OpenSimulator to Rust

[<- Back to docs](README.md) | [Architecture](architecture.md) | [Operator guide](operator-guide.md) | [Test grid deployment](test-grid-deployment.md)

Step-by-step guide for migrating an existing C# OpenSimulator 0.9.x grid to
the Rust implementation. The migration is incremental: C# and Rust services
can run side by side, and every step has a rollback path.

---

## Overview

### Why migrate

- **Performance:** static binaries with no .NET runtime; Rapier physics;
  async I/O throughout
- **Operational simplicity:** single-binary deployment, container-native,
  Prometheus metrics, structured logging
- **Security:** Luau script sandboxing with per-script memory limits, Argon2id
  password hashing, constant-time hash comparison
- **Modern tooling:** Cargo workspace, `sqlx` compile-time checked queries,
  `tracing` instrumentation

### What changes

| Area | C# | Rust |
|---|---|---|
| Runtime | .NET / Mono | Native static binary |
| Configuration | INI (Nini) | TOML with env overrides (`OPENSIM__Section__Key`) |
| Physics | ubODE / BulletSim | Rapier 3D |
| Scripts | XEngine / YEngine (LSL bytecode) | Luau VM (LSL transpiled to Luau) |
| Voice | Vivox / FreeSWITCH | WebRTC via wolfvoice sidecar |
| Database | MySQL.Data / Npgsql | sqlx (MariaDB, PostgreSQL, SQLite) |
| Monitoring | stats console, log4net | Prometheus `/metrics`, `RUST_LOG` |

### What stays the same

- **Wire protocol:** LLUDP, capabilities, XML-RPC login, all service HTTP
  endpoints -- fully wire-compatible (decision R2)
- **Database schema:** the 53 C# migration files are embedded and the Rust
  implementation reads the existing schema unchanged
- **OAR/IAR archives:** XML2 serialisation format is compatible; archives load
  without modification
- **Grid service API:** C# regions can talk to a Rust Robust, and Rust regions
  can talk to a C# Robust
- **Viewer compatibility:** Aurora Viewer (primary), plus any SL-compatible
  viewer for basic operations

---

## Pre-migration checklist

Before starting, verify and record:

- [ ] Current C# OpenSimulator version (must be 0.9.x)
- [ ] Database backend and version (MySQL/MariaDB, PostgreSQL, or SQLite)
- [ ] List of regions, their coordinates, and estate assignments
- [ ] List of user accounts
- [ ] Current `OpenSim.ini`, `Robust.ini`, `GridCommon.ini`, and any region
  override files
- [ ] OAR backups of all regions (take fresh ones now)
- [ ] IAR backups of any critical user inventories
- [ ] Note all custom OSSL settings (threat levels, per-function overrides)
- [ ] Note any external grid connections (hypergrid, OSgrid)
- [ ] Full database backup

> [!IMPORTANT]
> **Back up everything before proceeding.** The migration does not modify the
> C# database, but having a known-good backup is essential for rollback.

---

## Step 1: Deploy Rust Robust alongside C# Robust

Run the Rust `welkin-robust` on a different port from the C# Robust. This
lets you validate grid services without disrupting the live grid.

```sh
# C# Robust continues on port 8003 (its normal port)
# Start Rust Robust on port 8013
OPENSIM__Database__ConnectionString="<same-connection-as-csharp>" \
OPENSIM__Network__http_listener_port=8013 \
RUST_LOG=info \
  welkin-robust
```

Verify it starts and the health endpoint responds:

```sh
curl http://localhost:8013/health
# Expected: ok
```

The Rust Robust reads the same database as the C# Robust (read-only at this
point -- no writes until you point regions at it).

**Rollback:** stop `welkin-robust`. The C# grid is unaffected.

---

## Step 2: Validate database schema

Run the database validator to check that the existing schema matches what
the Rust implementation expects:

```sh
osim-migrate validate-db \
  --connection "Server=localhost;Port=3306;Database=opensim;User=opensim;Password=<password>"
```

The validator reports:

- **OK:** schema matches; proceed
- **Fixable differences:** run with `--generate-alters` to get ALTER TABLE
  statements, review them, and apply manually
- **Incompatible differences:** investigate; the schema may be from an older
  OpenSim version

```sh
# If fixable differences are found:
osim-migrate validate-db \
  --connection "..." \
  --generate-alters > schema-fixes.sql

# Review, then apply:
mysql -u opensim -p opensim < schema-fixes.sql
```

**Rollback:** the validator is read-only. The `--generate-alters` output is
only applied if you choose to run it, and the changes are standard schema
adjustments.

---

## Step 3: Convert configuration

Generate a Rust-compatible TOML configuration from your C# INI files:

```sh
osim-migrate convert-config \
  --input /path/to/Robust.ini \
  --output robust.toml

osim-migrate convert-config \
  --input /path/to/OpenSim.ini \
  --output region.toml
```

Review the generated TOML files. The converter handles:

- Section and key mapping from INI to TOML
- Service URL extraction
- Database connection string translation
- Grid profile detection

Settings that have no Rust equivalent are written as comments for your
reference. Settings that need manual attention are flagged with `# TODO`.

If you are on an external grid (e.g. OSgrid), also generate a grid profile:

```sh
osim-migrate convert-config \
  --input /path/to/GridCommon.ini \
  --output profiles/imported.toml
```

**Rollback:** configuration files are new files; they do not modify the C#
configuration.

---

## Step 4: Point one test region at Rust Robust

Choose a low-traffic test region. Stop it and restart it pointed at the Rust
Robust:

```sh
# Stop the C# test region (however you normally do this)

# Start a Rust region pointed at Rust Robust
OPENSIM__GridProfile__Name=own-grid \
OPENSIM__Database__ConnectionString="<connection>" \
OPENSIM__Network__http_listener_port=9010 \
OPENSIM__GridService__GridServerURI=http://localhost:8013 \
RUST_LOG=info \
  welkin-region
```

Log in with Aurora Viewer to the test region and verify:

- [ ] Login succeeds
- [ ] Avatar appears and moves
- [ ] Inventory loads
- [ ] Existing prims in the region appear correctly
- [ ] Chat works
- [ ] Teleport to a C# region works (if available)

**Rollback:** stop the Rust region, restart the C# region. It re-registers
with C# Robust and resumes where it left off.

---

## Step 5: Run conformance replay

Use `osim-conform` to replay recorded service fixtures against the Rust
Robust and check for protocol divergences:

```sh
osim-conform replay \
  --target http://localhost:8013 \
  --fixtures recordings/
```

The conformance harness replays HTTP, XML-RPC, and LLUDP exchanges and
reports differences. Any divergence should be investigated before proceeding.

Review the output:

- **Pass:** all replayed exchanges match
- **Divergence:** check whether the difference is cosmetic (whitespace, field
  ordering) or semantic (different values, missing fields)

**Rollback:** the replay tool is read-only.

---

## Step 6: Upgrade passwords (optional)

C# OpenSimulator stores password hashes as MD5. The Rust implementation
supports both MD5 (for compatibility) and Argon2id. To upgrade:

```sh
# Dry run first
osim-migrate upgrade-passwords \
  --connection "..." \
  --dry-run

# Then apply
osim-migrate upgrade-passwords \
  --connection "..." \
  --batch-size 100 \
  --rollback
```

The `--rollback` flag preserves the original MD5 hash in a backup column so
you can revert if needed.

> [!NOTE]
> Password upgrade is optional and can be done at any time. MD5 hashes
> continue to work with the Rust implementation. Argon2id is recommended for
> new grids or when security requirements justify the migration.

**Rollback:** with `--rollback`, the original MD5 hashes are preserved in a
backup column and can be restored.

---

## Step 7: Migrate all services to Rust Robust

Once testing on a single region is satisfactory, switch the Rust Robust to
the production port:

1. Stop C# Robust
2. Start Rust Robust on port 8003 (or whatever your standard port is)
3. Verify all regions re-register and login works

```sh
# Stop C# Robust

OPENSIM__Database__ConnectionString="<connection>" \
OPENSIM__Network__http_listener_port=8003 \
RUST_LOG=info \
  welkin-robust
```

At this point, all C# regions talk to Rust Robust. This is the mixed-grid
configuration (see Mixed-grid operation below).

**Rollback:** stop Rust Robust, restart C# Robust on port 8003. Regions
reconnect automatically.

---

## Step 8: Replace C# regions one at a time

Migrate regions from C# to Rust one at a time. For each region:

1. Stop the C# region process (or just that region if multi-region)
2. Start a Rust `welkin-region` for that region, pointed at Rust Robust
3. Verify the region loads, objects appear, and login works
4. Proceed to the next region

```sh
OPENSIM__GridProfile__Name=own-grid \
OPENSIM__Database__ConnectionString="<connection>" \
OPENSIM__Network__http_listener_port=9000 \
OPENSIM__GridService__GridServerURI=http://localhost:8003 \
RUST_LOG=info \
  welkin-region
```

The Rust region reads the same `regionstore` tables as the C# region. Objects,
terrain, and scene state carry over automatically.

**Rollback:** stop the Rust region, restart the C# region. The database is
shared and both read the same schema.

---

## Step 9: Import OAR archives for content that needs re-loading

If any content failed to load from the database (rare, but possible with
heavily customised regions), re-load it from OAR:

```sh
welkin-region load-oar /backups/MyRegion.oar
```

OAR archives from C# OpenSimulator 0.9.x load without modification.

**Rollback:** save a fresh OAR before loading, so you can revert the region
content.

---

## Step 10: Cut over DNS / load balancer

Once all regions are running on Rust:

1. Update DNS records if your grid hostname pointed at the C# server
2. Update any load balancer or reverse proxy configuration
3. Update viewer grid lists if you distribute them

Verify from outside your network:

```sh
curl https://grid.example.com/health
# Expected: ok
```

**Rollback:** revert DNS / load balancer changes.

---

## Mixed-grid operation

C# and Rust services can run simultaneously in several configurations:

### Rust Robust, C# regions

The most common intermediate state. Rust Robust serves all grid services;
C# regions connect to it via the standard HTTP/XML-RPC service endpoints.
This works because the wire format is compatible (decision R2, verified by
conformance replay).

### C# Robust, Rust regions

Also supported. Rust regions connect to C# Robust using the remote
connectors (`remote/asset.rs`, `remote/grid.rs`, `remote/inventory.rs`).

### Mixed regions

Some regions run C#, others run Rust, all pointed at the same Robust
(either C# or Rust). Teleports between C# and Rust regions work over the
standard simulation service protocol.

### Limitations of mixed operation

- Voice: WebRTC (Rust) and Vivox/FreeSWITCH (C#) are different systems;
  voice does not bridge between them
- Scripts: a script compiled for XEngine/YEngine does not run on Luau and
  vice versa; script state does not transfer between engines
- Physics: Rapier and ubODE/BulletSim produce slightly different results;
  objects at physics boundaries between Rust and C# regions may behave
  differently

---

## Post-migration verification checklist

After all services are running on Rust:

### Grid services

- [ ] Login works for all account types
- [ ] Inventory loads completely
- [ ] Asset fetch and upload work
- [ ] Friend lists load and presence updates appear
- [ ] Grid map shows all registered regions
- [ ] Profile pages load

### Region operations

- [ ] All regions appear on the map at correct coordinates
- [ ] Avatar movement, building, and scripting work in each region
- [ ] Teleport between regions works
- [ ] OAR save and load work
- [ ] Terrain edits persist across region restart
- [ ] NPC creation and control work

### Voice (if enabled)

- [ ] wolfvoice sidecar is running and healthy
- [ ] Voice icon appears in Aurora Viewer
- [ ] Spatial audio works

### Hypergrid (if enabled)

- [ ] Incoming teleports from remote grids succeed
- [ ] Outgoing teleports to remote grids succeed
- [ ] Foreign avatar inventory and appearance carry over

### Monitoring

- [ ] Prometheus scrapes both Robust and region metrics
- [ ] Health endpoints return `ok`
- [ ] Log output is captured (journald, file, or aggregator)

---

## FAQ

**Q: Can I migrate back to C# after switching?**

A: Yes. The database schema is shared, and C# OpenSimulator reads the same
tables. Stop the Rust services, start the C# services. The only
non-reversible change is password upgrade (Step 6), and even that is
reversible if you used `--rollback` to preserve the original hashes.

**Q: Do I need to change the database?**

A: No. The Rust implementation reads the existing C# schema. Run
`osim-migrate validate-db` to confirm; minor differences can be fixed with
the generated ALTER statements.

**Q: Will my OAR archives still work?**

A: Yes. The XML2 serialisation format is wire-compatible. OAR archives from
C# OpenSimulator 0.9.x load without modification.

**Q: What about my LSL scripts?**

A: LSL scripts need to be transpiled to Luau. The `welkin-script-compiler`
crate handles this, though it is still a skeleton. Hand-written Luau scripts
work now. Scripts compiled for XEngine/YEngine do not transfer directly.

**Q: Is the migration destructive?**

A: No. Every step is designed to be non-destructive. The Rust services read
the same database as C#, and no step modifies data in a way that prevents
C# from reading it. Take backups as a precaution.

**Q: How long does the migration take?**

A: The active migration (Steps 1-10) can be done in a maintenance window of
a few hours for a small grid. The key variable is how many regions you have
and how thoroughly you want to test each one. Steps 1-3 and 5-6 can be done
in advance without any downtime.

**Q: What about external grids like OSgrid?**

A: External grid mode works via grid profiles. Use `osim-migrate convert-config`
to import your `GridCommon.ini` as a custom profile. The external grid mode
has a CONDITIONAL PASS security review; see
[security-review-hypergrid.md](security-review-hypergrid.md) and the
[external grid mode summary](external-grid-mode-summary.md).

**Q: Do I need Aurora Viewer?**

A: Aurora Viewer (commit `3d523b14`) is the acceptance-tested viewer. Other
Second Life-compatible viewers work for basic operations. WebRTC voice
requires Aurora Viewer.

See also: [operator guide](operator-guide.md) for full configuration
reference, [test grid deployment](test-grid-deployment.md) for a quick trial.
