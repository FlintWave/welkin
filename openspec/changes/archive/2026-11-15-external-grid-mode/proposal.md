## Why

A region simulator should be able to join a grid somebody else runs. OSgrid
is the largest such grid and the first target. Operators must also be able
to choose a private grid, a standalone world, or any other public grid, so
the grid a region joins has to be configuration, not code.

## What Changes

- **Grid profiles.** One TOML file names every service a region talks to
  and how. Profiles shipped: `standalone`, `own-grid` (the Rust `robust`),
  `osgrid`, and `custom` (any grid, by its service addresses). Hypergrid is
  a switch inside a profile.
- **The full set of remote connectors** a C# region uses in grid mode
  (`bin/config-include/Grid.ini`): assets with a local cache, inventory,
  grid, avatar, authentication, presence, user accounts, agent preferences,
  grid user, simulation, land, map images, mute list and neighbours, plus
  the hypergrid variants in `GridHypergrid.ini`.
- **Region registration** as the C# connector does it (`METHOD=register`
  with protocol version 1), including deregistration on shutdown.
- **The OSgrid profile is generated, not hand-written.** OSgrid publishes a
  preconfigured build whose `GridCommon.ini` already points at its
  `*.osgrid.org` services. The configuration importer reads that file and
  writes the profile, so addresses are never copied from memory and can be
  refreshed when OSgrid changes them.
- **OSgrid rules built into the wizard checks**, from a 2026 setup guide:
  region names are unique across the grid; the estate owner must be the
  operator's OSgrid avatar; map coordinates must be free; each region needs
  its own port open for TCP and UDP; the external host name must be the
  public address.
- Per-profile overrides: physics engine, script limits, voice, search,
  groups, profiles and offline messaging endpoints.

## Impact

- Depends on `region-simulator-core`, `hypergrid` and the importer in
  `port-foundation`.
- Not established, and blocking for OSgrid specifically: whether OSgrid
  accepts regions run on software other than its own build. Its FAQ could
  not be read (the site refused automated access). The OSgrid
  administrators must be asked before any region is attached.
- Risk: HIGH. A region on a public grid writes to other people's inventory
  and to a shared asset store. A fault could damage content that is not the
  operator's. The profile stays disabled until the same build has passed
  the full conformance suite against both a C# grid and a Rust grid, and it
  gets an independent second review.
