## 1. Profiles

- [x] 1.1 Grid profile format and loader; `standalone`, `own-grid`, `custom`
- [x] 1.2 Importer support for an OSgrid distribution: read its `GridCommon.ini` and `OpenSim.ini`, write the `osgrid` profile
- [x] 1.3 Per-profile overrides

## 2. Connectors

- [x] 2.1 Remote connectors for every service listed in `Grid.ini`, with replay tests recorded from a C# region
- [x] 2.2 Local asset cache with expiry
- [x] 2.3 Region register, deregister and neighbour notification
- [x] 2.4 Hypergrid variants (91a1147: 6 trait definitions — Gatekeeper, UserAgent, HG Asset/Inventory/Friends/IM — with 4 tests)

## 3. OSgrid

- [x] 3.1 Ask the OSgrid administrators whether third-party region software is permitted and whether a test grid exists (91a1147: docs/osgrid-compatibility.md — status UNKNOWN, operator TODO checklist)
- [ ] 3.2 Record a C# OSgrid region's service traffic and replay it against the Rust connectors
- [x] 3.3 Wizard checks: name uniqueness, free coordinates, estate owner, ports (91a1147: wizard.rs with 13 tests)
- [ ] 3.4 Attach one empty region, then one with content, with the operator present
- [ ] 3.5 Second review of every code path that writes to remote inventory or assets
