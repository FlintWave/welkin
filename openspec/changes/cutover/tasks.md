## 1. Migration tooling

- [ ] 1.1 Database migration validator: connect to existing C# grid database, verify schema compatibility, report any column/type mismatches, generate ALTER statements if needed
- [ ] 1.2 Password upgrade tool: batch Argon2id hash generation over existing MD5 digests, with rollback flag to keep MD5 readable by C# during mixed operation
- [ ] 1.3 Configuration converter: parse C# OpenSim.ini + Robust.ini, generate equivalent Rust configuration, flag unsupported options, warn on deprecated settings

## 2. Bot framework

- [ ] 2.1 pCampBot port: login, walk, fly, teleport, cross regions, chat, group join using aurora-net viewer protocol. Configurable avatar count, action intervals, region targets
- [ ] 2.2 Bot scripting: programmable action sequences for regression testing — rez object, edit, take, give, set permissions, run script, verify output

## 3. Protocol conformance

- [ ] 3.1 Full protocol replay: replay all recorded C# LLUDP and capability request/response pairs against Rust server, diff outputs, categorize divergences as bug/intentional/cosmetic
- [ ] 3.2 Service connector conformance: replay C# Robust HTTP requests against Rust Robust, verify response format and content match

## 4. Load testing

- [ ] 4.1 Load test harness: configurable concurrent avatars, physics objects, active scripts, asset transfers per region. Measure: FPS, script EPS, memory, network bandwidth
- [ ] 4.2 Baseline measurements: run load tests against C# OpenSim on identical hardware, record targets
- [ ] 4.3 Rust load tests: run same load tests against Rust server, compare against C# baselines, file issues for any regression

## 5. Mixed-grid validation

- [ ] 5.1 C# region against Rust Robust: configure C# simulator to connect to Rust grid services, validate login, teleport, inventory, groups, IM, map
- [ ] 5.2 Rust region against C# Robust: configure Rust simulator to connect to C# Robust, validate same operations
- [ ] 5.3 Cross-region teleport in mixed grid: C#→Rust and Rust→C# region crossings with inventory, attachments, scripts

## 6. Container hardening

- [ ] 6.1 Production Docker images: distroless base, non-root user, health check endpoints, graceful shutdown (SIGTERM handling), configurable log level
- [ ] 6.2 Docker Compose production stack: Robust + region + wolfvoice + database, backup cron (scheduled OAR save), log rotation, Prometheus metrics endpoint

## 7. First production grid and documentation

- [ ] 7.1 Test grid deployment: all-Rust services and regions, OAR-imported content, Aurora Viewer login, public access for testing
- [ ] 7.2 Operator guide: installation, configuration reference, backup/restore, monitoring, troubleshooting
- [ ] 7.3 Migration guide: step-by-step C#-to-Rust migration, rollback procedures, mixed-grid operation, DNS cutover
- [ ] 7.4 FINAL REVIEW: end-to-end acceptance — Aurora Viewer session covering login, chat, teleport, inventory, groups, voice, HG travel, OAR save/load, estate management (Opus reviewer)
