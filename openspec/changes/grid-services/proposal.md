## Why

The grid services are the safest place to start replacing C# with Rust. They
are HTTP and XML-RPC handlers over a database (`Services`, `Server` and
`Data`, 74,749 lines), and simulators reach them only through connector
protocols. A Rust service can therefore run under unmodified C# simulators,
one service at a time, with the C# service as the fallback.

## What Changes

- `data`: the existing schema reached through `sqlx` for MariaDB/MySQL,
  PostgreSQL and SQLite, with the C# migrations carried over so a database
  can be opened by either implementation.
- `robust` binary hosting, each switchable on or off: login, grid, user
  accounts, authentication, authorization, assets (including the
  file-system asset store), inventory, presence, grid user, friends, avatar,
  map images, estate, mute list, user profiles, simulation.
- The same URL paths, form encodings and reply formats as the C# handlers in
  `OpenSim/Server/Handlers`.
- Groups, OfflineIM and Hypergrid services are separate later changes.

## Impact

- Order of work: assets and inventory first (highest traffic, simplest
  semantics), login last (touches every other service).
- Risk: HIGH for authentication and for anything that writes inventory or
  assets. A fault here loses user content or weakens login. These get an
  independent second review and are tested against a copy of a real grid
  database before any production use.
