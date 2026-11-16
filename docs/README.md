# Welkin

Rust reimplementation of [OpenSimulator 0.9.3.0](http://opensimulator.org/)
grid services and region simulator. It aims to match the C# server on the wire
and in its database schema, so existing grids can migrate without losing content.

Your existing grid, content and viewer, on a server that is easier to run and
harder to break.

**Status:** alpha (v0.1.0-alpha). Welkin builds, runs, and lets a viewer log in
to a single-region grid on SQLite. It is not ready for a production grid. See
the [viewer compatibility page](viewer-compatibility.md) for what works and what
does not.

This book has two parts.

## Operator guide

For people who run a grid or a region.

- [Quick start](quick-start.md): build, configure and run
- [Configuration reference](configuration.md): every `welkin.toml` and `robust.toml` key
- [Database setup](database-setup.md): SQLite (verified), MariaDB and PostgreSQL (untested)
- [Regions, estate and users](regions-estate-users.md): the region console, estate owner, accounts
- [ROBUST grid services server](robust-server.md), [Container deployment](container-guide.md),
  [Operator guide](operator-guide.md), [Test grid deployment](test-grid-deployment.md)
- [Migration guide](migration-guide.md): C# to Rust, with rollback at every step
- [Viewer compatibility](viewer-compatibility.md) and [OSgrid compatibility](osgrid-compatibility.md)

## Agent build guide

For people and AI agents who develop Welkin.

- [Architecture](architecture.md) and [Crate structure](crate-structure.md)
- [Module development guide](module-development.md): write a region module
- [Script engine](scripting.md) and [OSSL reference](ossl.md)
- [Testing](testing.md): cargo tests, CI, golden vectors, bots, the LibreMetaverse harness
- [OpenSpec workflow](openspec-workflow.md) and [Task system](task-system.md)
- [Decision log](decisions.md)

## Phase summaries

| Phase | Summary | Security review |
|---|---|---|
| 0 | [Foundation and audit](phase-0-summary.md) | — |
| 1 | [Protocol core](protocol-core-summary.md) | — |
| 1 | [Physics backend](physics-backend-summary.md) | — |
| 2 | [Grid services](grid-services-summary.md) | — |
| 2 | [Capacity assessor](capacity-assessor-summary.md) | — |
| 2 | [Grid portal](grid-portal-summary.md) | — |
| 3 | [Region simulator core](region-simulator-core-summary.md) | — |
| 3 | [Prim meshing](prim-meshing-summary.md) | — |
| 4 | [Script engine](script-engine-summary.md) | [Sandboxing](security-review-script-sandboxing.md) |
| 4 | [OSSL API](ossl-api-summary.md) | [OSSL API](security-review-ossl-api.md) |
| 5 | [Region modules](region-modules-summary.md) | [Permissions](security-review-permissions.md) |
| 5 | [Archives](archives-summary.md) | [Archives](security-review-archives.md) |
| 6 | [Social modules](social-modules-summary.md) | [Social modules](security-review-social-modules.md) |
| 6 | [External grid mode](external-grid-mode-summary.md) | [Remote writes](security-review-remote-writes.md) |
| 6 | [Hypergrid](hypergrid-summary.md) | [Hypergrid](security-review-hypergrid.md) |
| 6 | [WebRTC voice](webrtc-voice-summary.md) | [Voice](security-review-voice.md) |
| — | [Object interaction](object-interaction-summary.md) | — |
| — | [Module activation](module-activation-summary.md) | — |
| — | [Viewer messaging](viewer-messaging-summary.md) | — |
| — | [Currency module](currency-module-summary.md) | — |
| — | [Remaining parity](remaining-parity-summary.md) | — |
| — | [Cutover](cutover-summary.md) | — |

## Reference

- [Licence audit](licence-audit.md): C# dependencies, compatibility, dropped items
- [LSL corpus analysis](lsl-corpus-analysis.md): function frequency in real scripts
- [Naming and trademarks](naming-and-trademarks.md)
- [Script engine spike results](script-engine-spike-results.md)
- [Final acceptance review](final-acceptance-review.md)

## Security reviews

All security-critical phases receive an adversarial Opus review before
acceptance. Current verdicts:

| Review | Verdict | Critical | High | Medium |
|---|---|---|---|---|
| [Permissions](security-review-permissions.md) | CONDITIONAL PASS | 1 | 3 | 4 |
| [OSSL API](security-review-ossl-api.md) | CONDITIONAL PASS | 0 | 1 | 2 |
| [Script sandboxing](security-review-script-sandboxing.md) | CONDITIONAL PASS | 0 | 2 | 4 |
| [Remote writes](security-review-remote-writes.md) | CONDITIONAL PASS | 1 | 4 | 6 |
| [Archives](security-review-archives.md) | CONDITIONAL PASS | 0 | 0 | 5 |
| [Social modules](security-review-social-modules.md) | CONDITIONAL PASS | 0 | 3 | 10 |
| [Hypergrid](security-review-hypergrid.md) | CONDITIONAL PASS | 3 | 3 | 3 |
| [Voice](security-review-voice.md) | CONDITIONAL PASS | 0 | 2 | 3 |

> [!WARNING]
> All phases carry a CONDITIONAL PASS. The tracked findings must be resolved
> before production deployment. See each review for specifics.

## Building the docs

These docs are plain GitHub-Flavored Markdown and render on GitHub with no
tooling. To build a static site locally:

```sh
cargo install mdbook mdbook-mermaid mdbook-admonish
cd docs
mdbook build
# open book/index.html
```
