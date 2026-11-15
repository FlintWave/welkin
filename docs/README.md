# Welkin

Rust reimplementation of [OpenSimulator 0.9.3.0](http://opensimulator.org/)
grid services and region simulator. Wire and schema compatible with the C#
server, so existing grids can migrate without losing content.

Your existing grid, content and viewer, on a server that is easier to run and
harder to break.

**Status:** Pre-alpha. First successful viewer connection achieved (Aurora
Viewer and Firestorm). Working toward full viewer compatibility.

> [!NOTE]
> Start here: [Architecture](architecture.md) gives the workspace layout,
> crate map, dependency graph and build instructions.

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

## Operator documentation

- [Test grid deployment](test-grid-deployment.md) — deploy a containerised test grid in minutes
- [Operator guide](operator-guide.md) — installation, configuration, running, monitoring, security
- [Migration guide](migration-guide.md) — step-by-step C# to Rust migration with rollback at every step

## Reference

- [Architecture](architecture.md) — workspace, crates, dependency graph, build
- [Decision log](decisions.md) — every design decision with rationale
- [Licence audit](licence-audit.md) — C# dependencies, compatibility, dropped items
- [LSL corpus analysis](lsl-corpus-analysis.md) — function frequency in real scripts
- [Naming and trademarks](naming-and-trademarks.md)
- [OSgrid compatibility](osgrid-compatibility.md)

## Security reviews

All security-critical phases receive an adversarial Opus review before
acceptance. Current verdicts:

| Review | Verdict | Critical | High | Medium |
|---|---|---|---|---|
| [Permissions](security-review-permissions.md) | CONDITIONAL PASS | 1 | 3 | 4 |
| [OSSL API](security-review-ossl-api.md) | CONDITIONAL PASS | 0 | 1 | 2 |
| [Script sandboxing](security-review-script-sandboxing.md) | CONDITIONAL PASS | 0 | 2 | 0 |
| [Remote writes](security-review-remote-writes.md) | CONDITIONAL PASS | 1 | 4 | 0 |
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
