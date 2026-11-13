## 1. Profiles

- [ ] 1.1 Grid profile format and loader; `standalone`, `own-grid`, `custom`
- [ ] 1.2 Importer support for an OSgrid distribution: read its `GridCommon.ini` and `OpenSim.ini`, write the `osgrid` profile
- [ ] 1.3 Per-profile overrides

## 2. Connectors

- [ ] 2.1 Remote connectors for every service listed in `Grid.ini`, with replay tests recorded from a C# region
- [ ] 2.2 Local asset cache with expiry
- [ ] 2.3 Region register, deregister and neighbour notification
- [ ] 2.4 Hypergrid variants

## 3. OSgrid

- [ ] 3.1 Ask the OSgrid administrators whether third-party region software is permitted and whether a test grid exists
- [ ] 3.2 Record a C# OSgrid region's service traffic and replay it against the Rust connectors
- [ ] 3.3 Wizard checks: name uniqueness, free coordinates, estate owner, ports
- [ ] 3.4 Attach one empty region, then one with content, with the operator present
- [ ] 3.5 Second review of every code path that writes to remote inventory or assets
