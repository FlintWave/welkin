<!-- SPDX-License-Identifier: LGPL-3.0-or-later -->
# Database setup

[← Back to docs](index.md) · [Configuration reference](configuration.md) · [Operator guide](operator-guide.md)

Welkin stores objects, terrain, estate settings, accounts, inventory and groups
in a SQL database. The region and the grid services each take a database URL.

> [!WARNING]
> **SQLite is the only database that has been run and verified.** The code also
> supports MariaDB, MySQL and PostgreSQL, but no one has run the region or grid
> services against them yet, and some problems are known. Use SQLite for the
> alpha release.

## SQLite (verified)

SQLite needs no server. Point both programs at files:

```toml
# robust.toml
[DatabaseService]
ConnectionString = "sqlite:robust.db?mode=rwc"
```

```toml
# welkin.toml
[DatabaseService]
RegionConnectionString = "sqlite:region.db?mode=rwc"
RobustConnectionString = "sqlite:robust.db?mode=rwc"
```

`?mode=rwc` creates the file if it does not exist. Both programs create their
tables on first start: the region runs its migrations at every start, and
`welkin-robust` creates its tables for SQLite itself. To back up, stop both
programs and copy the `.db` files. The region's `backup` console command saves
pending object changes to the database first.

## MariaDB, MySQL and PostgreSQL (untested)

| Backend | URL format |
|---|---|
| MariaDB or MySQL | `mysql://user:pass@host/db` |
| PostgreSQL | `postgres://user:pass@host/db` |

Known problems, so you can judge the risk:

- The `welkin-data` crate still has about 158 hardcoded `$n` placeholders, which
  work on PostgreSQL and SQLite but not on MariaDB. A sweep to make them
  backend-aware is pending.
- The `script_states` table stores scripts in a `BLOB` column, which needs
  `BYTEA` on PostgreSQL and `LONGBLOB` on MariaDB.
- The `loc_x` and `loc_y` columns of the region identity table need `BIGINT` on
  PostgreSQL (issue #79).

The [operator guide](operator-guide.md) has server-side setup steps for MariaDB
and PostgreSQL. Treat them as the intended setup, not a verified one. If you try
either backend, please report what breaks.

## What the region saves

The region saves changed objects and edited terrain on a timer
(`[Backup] IntervalSeconds`, 900 seconds by default) and loads them at start,
together with its estate settings. Everything is keyed by the region's UUID, so
see [Regions, estate and users](regions-estate-users.md) before you rename or
move a region.
