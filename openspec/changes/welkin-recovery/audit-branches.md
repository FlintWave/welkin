# Branch, stash and PR audit (2026-11-22)

Where code was sitting outside a reviewed PR, and which merged PRs were too large to review.

## Branches

| Branch | Own commits vs `origin/main` | Content not on main | Action |
|---|---|---|---|
| `dev` (local and origin) | 57 (unsquashed history of PR #31/#33) | None. Tree differs from main in 2 files only: main has the `#[allow(dead_code)]` additions from `e56e33a`/`37a2d86`. | Tagged `pre-squash-history`. Keep for forensics; delete the branch after WP 0.15. |
| `dev-clean` (local and origin) | 1: `2492ef1` glTF parser | `crates/welkin-prim/src/gltf.rs` and its Cargo changes. Plus uncommitted `pub mod gltf;`, a Cargo.lock change, and an edit to the orphan root `src/mesh.rs`. | WP 0.6: re-land as its own PR from main (or drop). Discard the `src/mesh.rs` edit. Retire the branch afterwards. |
| `realmwalker-phase3` (local, worktree) | 10 | None. All 10 commits are ancestors of `dev`, so the content reached main through PR #33. Includes the fake "real implementations" `6523bd6`, `61723e3` and `a45ad61`. | Covered by WP 0.15. Delete the branch and worktree after the audit. |
| `df-Implement_glTF_parser__…-1794993517357` (local, worktree) | 56 | None. Contained in `dev`. | Delete with its worktree. |
| `df-Implement_glTF_parser__…-1795043209164` (local, worktree) | 1 | Same commit as `dev-clean` (`2492ef1`). | Delete with its worktree. |
| `main` (local) | 0 (was 196 behind) | n/a | Fast-forwarded to `origin/main`. |

## Stashes

| Stash | Date | Content | Action |
|---|---|---|---|
| `stash@{0}` (on dev) | 11-16 | Day-cycle values computed and discarded every frame, and `GodUpdateRegionInfo` persistence through an unsound `unsafe` cast of `Arc<Scene>` to `*mut Scene`. | Do not land. The unsafe cast pattern is a defect to check for in WP 0.15. |
| `stash@{1}` (on dev) | 11-16 | Realmwalker subagent partial progress. About 1,046 lines across 16 files, including three files that exist nowhere else: `welkin-net/src/agent_export.rs` (cross-region agent handoff), `welkin-realmwalker/src/inventory.rs`, `welkin-services/src/remote/network.rs` (llHTTPRequest URL filter). | Preserved on branch `salvage/realmwalker-partial-2026-11-16` (commit `54ae1e4`). Not for merging as-is. Inputs to Phase 2 (Simulation service), the realmwalker inventory tests, and Phase 8 (HTTP egress). |
| `stash@{2}`-`stash@{6}` | 11-13/14 | Before the crate rename from `opensim-*` to `welkin-*`, and the old proposals since archived. | Obsolete. Leave in place; no action. |

## Oversized merged PRs

All 33 PRs were merged with zero reviews. Five are far beyond a reviewable size or mix unrelated concerns:

| PR | Size | Problem | Remediation |
|---|---|---|---|
| #29 Release v0.1.0-alpha | +22,420/-1,053, 258 files | A release cut that bundled weeks of feature work. | WP 0.15 audit split by crate, one issue per crate area. |
| #33 Consolidate dev integration work | +12,138/-507, 157 files | Squash of the dark factory and task runner. Orphan code, fake tests, the hidden login throttle. | WP 0.3, 0.5, 0.6 and 0.15. Per-commit history is at `pre-squash-history`. |
| #31 Sprint: egress, scripting, portable SQL, visible effects | +6,998/-1,107, 64 files | Four unrelated features in one PR. | WP 0.15 audit split by feature. |
| #5 Add full documentation content | +12,044, 58 files | Docs only, but too large to review. Many claims are now false. | Covered by WP 0.14 and the docs pass. |
| #1 Repo setup | +373/-674, 7 files | A license change mixed with community health files, and the license was changed again in #4. | Historical. No action beyond checking that LICENSE and SPDX headers agree (WP 0.15). |

Rule from now on: one concern per PR, under about 800 changed lines (excluding lockfiles and generated files). Larger changes are split before review.
