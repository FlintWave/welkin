# cutover: Migration tooling, load tests, first production grid

## Summary

The final phase: validate the Rust implementation against real-world usage,
provide migration tooling for existing C# grids, run load tests, and bring
up the first production grid. This phase does not port new C# code — it
verifies that everything ported so far works end-to-end.

## Scope

### In scope

- **Migration tooling**: database migration scripts for grids moving from C#
  OpenSimulator to Rust. The schema is compatible, but Argon2id password
  upgrade (from salted MD5), asset storage migration (gzip→zstd), and
  configuration conversion (INI→TOML or compatible INI) need tooling.
- **pCampBot port**: port the bot framework (~2,500 lines) for automated
  testing — login, walk, fly, teleport, cross regions, chat, group join.
- **Load testing**: multi-region stress tests with concurrent avatars,
  physics objects, scripts, and asset transfers. Target: C# parity at
  minimum, measured on identical hardware.
- **Protocol conformance**: replay all recorded C# request/response pairs
  against the Rust server. Every divergence is a bug or a documented
  intentional change.
- **Mixed-grid validation**: C# simulators running against Rust Robust
  services, and Rust regions running against C# Robust. Both directions
  must work for incremental migration.
- **Container hardening**: production Docker images (distroless base),
  health checks, graceful shutdown, log rotation, backup cron.
- **First production grid**: a test grid open to Aurora Viewer users,
  running all Rust services and regions, with OAR-imported content.
- **Documentation**: operator guide, migration guide, troubleshooting.

### Out of scope

- Performance optimization beyond C# parity (future work).
- Kubernetes/Helm charts (future, operator-contributed).
- Monitoring dashboards (Prometheus/Grafana templates provided but not
  a managed service).

## Dependencies

All other phases must be accepted before cutover begins:
- archives (OAR import for content migration)
- social-modules (groups, profiles for user migration)
- hypergrid (cross-grid for mixed-grid validation)
- webrtc-voice (voice for full feature parity)

## Design

### Migration path

```
1. Deploy Rust Robust alongside C# Robust (different port)
2. Point one test region at Rust Robust → validate services
3. Run pCampBot + protocol replay → find divergences
4. Migrate all services to Rust Robust
5. Replace C# regions one at a time with Rust regions
6. Import OAR archives for content migration
7. Cut over DNS/load balancer
```

### Key decisions

- C1: Schema stays compatible throughout — no breaking migrations. The
  Argon2id password upgrade is opt-in and preserves MD5 fallback.
- C2: pCampBot ported in Rust using `aurora-net` for viewer protocol,
  not as a separate binary. Reuses the existing test infrastructure.
- C3: Load test targets defined against C# measurements on identical
  hardware. The Rust server must meet or exceed C# on: avatars per region,
  script executions per second, asset throughput, region crossing latency.
- C4: Mixed-grid is the primary migration strategy. No big-bang cutover.
