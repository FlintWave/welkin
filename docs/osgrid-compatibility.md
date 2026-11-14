# OSgrid Compatibility Notes

SPDX-License-Identifier: GPL-3.0-or-later

Status: research from source code only. **No direct contact with OSgrid
administrators has been made.** This document records what the C# codebase
and the project's own OpenSpec proposals reveal about running a non-C#
region on OSgrid.

## What is OSgrid

OSgrid (<https://www.osgrid.org>) is the largest public OpenSimulator grid.
It is referenced throughout the C# source as both a development partner and
the primary external-grid target:

- `CONTRIBUTORS.txt` credits Nebadon Izumi (Michael Cerquoni, OSgrid) as
  a contributor.
- `bin/LaunchSLClient.ini` ships with `loginURI = http://www.osgrid.org:8002/`
  as a preconfigured grid.
- `bin/Robust.HG.ini.example` shows `login.osgrid.org` as a sample
  `GatekeeperURIAlias`.

## Third-party region software: status unknown

The project's own `openspec/changes/external-grid-mode/proposal.md` states:

> Not established, and blocking for OSgrid specifically: whether OSgrid
> accepts regions run on software other than its own build.  Its FAQ could
> not be read (the site refused automated access).  The OSgrid
> administrators must be asked before any region is attached.

The `openspec/ROADMAP.md` repeats this as an open question:

> OSgrid: whether it accepts regions run on third-party software.  Its FAQ
> refused automated access; the administrators must be asked.

**No evidence was found in the codebase that OSgrid either permits or
prohibits third-party region implementations.** The C# grid service enforces
protocol-version checks and duplicate-name/coordinate checks during
registration (`OpenSim/Services/GridService/GridService.cs`), but these are
generic to all grids, not OSgrid-specific policies.

## OSgrid setup requirements (from codebase documentation)

The `external-grid-mode` proposal documents rules derived from a 2026 OSgrid
setup guide:

1. **Region names must be unique** across the entire grid. The C# grid
   service rejects registration with `"Duplicate region name"` when
   `AllowDuplicateNames` is false (the default).
2. **Map coordinates must be free.** The grid service checks for overlapping
   regions at the requested location and rejects with `"Region overlaps
   another region"`.
3. **The estate owner must be the operator's OSgrid avatar.** The C# code
   verifies the `PrincipalID` against the authentication service when the
   `Authenticate` flag is set.
4. **Each region needs its own port open for TCP and UDP.** The region
   registers `serverPort` and `serverHttpPort`; external reachability is the
   operator's responsibility.
5. **The external hostname must be the public address.** The `serverURI`
   field in the registration must resolve to the region from outside.

## OSgrid service addresses

OSgrid publishes a preconfigured OpenSimulator distribution whose
`GridCommon.ini` points at `*.osgrid.org` services. The Rust port's design
generates the OSgrid profile by importing that file rather than hard-coding
addresses, so they stay current when OSgrid changes them.

Known addresses from the C# source:

| Purpose | Address |
|---|---|
| Login URI | `http://www.osgrid.org:8002/` |
| Gatekeeper alias | `login.osgrid.org` |

The remaining service URIs (asset, inventory, grid, presence, etc.) are in
the OSgrid-distributed `GridCommon.ini`, which is not included in the
upstream OpenSimulator source.

## Resource guidance from OSgrid community

From `openspec/changes/capacity-assessor/proposal.md`:

- About 1 GB of memory and one core per region (OSgrid wiki tutorial).
- 4 regions per simulator, 16 per server (OSgrid forum post).

## Test grid

No evidence of a dedicated OSgrid test or staging grid was found in the
codebase. The `external-grid-mode` tasks list (`tasks.md` item 3.4) plans
to attach first an empty region, then one with content, "with the operator
present" - implying testing happens on the live grid under supervision.

## TODO for the operator

- [ ] Contact OSgrid administrators to ask:
  1. Whether third-party region software (not a C# OpenSimulator build) is
     permitted to connect to OSgrid.
  2. Whether a test/staging grid exists for development.
  3. Whether any additional registration or approval process is required
     beyond the standard region-registration protocol.
- [ ] Obtain a current OSgrid distribution and run the configuration
  importer to generate the `osgrid` grid profile.
- [ ] Pass the full conformance suite against both a C# grid and a Rust
  grid before enabling the OSgrid profile (per the proposal's safety
  requirement).
