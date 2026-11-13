## 1. Repository

- [ ] 1.1 Create the repository and workspace skeleton with empty crates
- [ ] 1.2 Move `openspec/` into it; add the GPLv3 `LICENSE` and a `NOTICE` carrying the OpenSimulator BSD notice, contributor list and Aurora Viewer notices
- [ ] 1.3 CI: fmt, clippy, test, `cargo deny`, image build

## 2. Runtime base

- [ ] 2.1 TOML configuration with environment overrides
- [ ] 2.2 Importer for `OpenSim.ini`, `Robust.ini` and `Regions/*.ini`
- [ ] 2.3 Logging, metrics endpoint, health endpoint, graceful shutdown

## 3. Containers

- [ ] 3.1 Multi-stage `Containerfile`, static binaries, non-root
- [ ] 3.2 Compose file: database, `robust`, one region, with UDP port guidance
- [ ] 3.3 Run the unmodified C# server in a container as the reference

## 4. Conformance harness

- [ ] 4.1 HTTP and XML-RPC recorder and replayer
- [ ] 4.2 LLUDP capture and replay
- [ ] 4.3 `pCampBot` wrapper with pass or fail output
