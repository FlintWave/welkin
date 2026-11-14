## 1. Migration tooling

- [x] 1.1 Database migration validator: connect to existing C# grid database, verify schema compatibility, report any column/type mismatches, generate ALTER statements if needed (opensim-migration/db_validator.rs 863 lines, 15 tests)
- [x] 1.2 Password upgrade tool: batch Argon2id hash generation over existing MD5 digests, with rollback flag to keep MD5 readable by C# during mixed operation (opensim-migration/password_upgrade.rs 459 lines, 12 tests)
- [x] 1.3 Configuration converter: parse C# OpenSim.ini + Robust.ini, generate equivalent Rust configuration, flag unsupported options, warn on deprecated settings (opensim-migration/config_converter.rs 517 lines, 12 tests)

## 2. Bot framework

- [x] 2.1 pCampBot port: login, walk, fly, teleport, cross regions, chat, group join using aurora-net viewer protocol. Configurable avatar count, action intervals, region targets (opensim-conformance/pcampbot.rs rewritten, 16 tests)
- [x] 2.2 Bot scripting: programmable action sequences for regression testing — rez object, edit, take, give, set permissions, run script, verify output (opensim-conformance/bot_scripting.rs 534 lines, 10 tests)

## 3. Protocol conformance

- [x] 3.1 Full protocol replay: replay all recorded C# LLUDP and capability request/response pairs against Rust server, diff outputs, categorize divergences as bug/intentional/cosmetic (opensim-conformance/protocol_replay.rs, 14 tests)
- [x] 3.2 Service connector conformance: replay C# Robust HTTP requests against Rust Robust, verify response format and content match (opensim-conformance/service_conformance.rs, 10 tests)

## 4. Load testing

- [x] 4.1 Load test harness: configurable concurrent avatars, physics objects, active scripts, asset transfers per region. Measure: FPS, script EPS, memory, network bandwidth (opensim-conformance/load_test.rs 795 lines, 15 tests)
- [x] 4.2 Baseline measurements: run load tests against C# OpenSim on identical hardware, record targets (load_test.rs save_baseline/load_baseline + ComparisonReport)
- [x] 4.3 Rust load tests: run same load tests against Rust server, compare against C# baselines, file issues for any regression (load_test.rs compare_baselines with per-metric pass/fail)

## 5. Mixed-grid validation

- [x] 5.1 C# region against Rust Robust: configure C# simulator to connect to Rust grid services, validate login, teleport, inventory, groups, IM, map (opensim-conformance/mixed_grid.rs validate_csharp_region_rust_robust, 17 tests)
- [x] 5.2 Rust region against C# Robust: configure Rust simulator to connect to C# Robust, validate same operations (mixed_grid.rs validate_rust_region_csharp_robust)
- [x] 5.3 Cross-region teleport in mixed grid: C#→Rust and Rust→C# region crossings with inventory, attachments, scripts (mixed_grid.rs validate_cross_region_teleport)

## 6. Container hardening

- [x] 6.1 Production Docker images: distroless base, non-root user, health check endpoints, graceful shutdown (SIGTERM handling), configurable log level (Dockerfile 283 lines, Dockerfile.region 157 lines)
- [x] 6.2 Docker Compose production stack: Robust + region + wolfvoice + database, backup cron (scheduled OAR save), log rotation, Prometheus metrics endpoint (docker-compose.prod.yml 253 lines, .env.example, .dockerignore)

## 7. First production grid and documentation

- [x] 7.1 Test grid deployment: all-Rust services and regions, OAR-imported content, Aurora Viewer login, public access for testing (docs/test-grid-deployment.md 215 lines)
- [x] 7.2 Operator guide: installation, configuration reference, backup/restore, monitoring, troubleshooting (docs/operator-guide.md 606 lines)
- [x] 7.3 Migration guide: step-by-step C#-to-Rust migration, rollback procedures, mixed-grid operation, DNS cutover (docs/migration-guide.md 477 lines)
- [ ] 7.4 FINAL REVIEW: end-to-end acceptance — Aurora Viewer session covering login, chat, teleport, inventory, groups, voice, HG travel, OAR save/load, estate management (Opus reviewer)
