# Welkin recovery

**Status:** Active. Supersedes the status claims in `openspec/ROADMAP.md` and `ROADMAP.md` until Phase 0 task 0.14 rewrites them.
**Priority:** Highest. No other change is worked on until Phase 0 is done.

## Why

An autonomous task runner (2026-11-16) and a "dark factory" (2026-11-17 to 2026-11-18) pushed a large amount of unreviewed work to main. PRs #29, #31 and #33 together add about 41,000 lines in three squash merges, with no reviews. A read-only assessment on 2026-11-22 found:

- Code written outside any Cargo package, which therefore never compiled.
- Fake implementations and hollow tests.
- A grid-wide login lockout, added in a commit titled "no code behavior changes".
- Unauthenticated circuits and capabilities.
- About 84% of a sample of ticked task boxes contradicted by the code.

`design.md` holds the full assessment and the phased plan. `audit-branches.md` records where stray code and oversized PRs were found.

## What changes

- **Phase 0, stabilise:** security hotfixes; delete the fake work; honest CI gates; required reviews; reset the task boxes to what the code really does.
- **Phase 1, backbone:** packet dispatcher, scene event manager, module host, service connectors, a reference C# grid for interop tests.
- **Phases 2-10:** parity with OpenSimulator 0.9.3.0 in dependency order. Each phase ends with something a real viewer or the realmwalker bot can do.
- **Phase 11:** the roadmap's extra features.

## Rules for this change

- One work package is one branch and one PR, under about 800 changed lines, excluding lockfiles and generated files. Mixed-concern PRs are split before review.
- A box is ticked only with an `Evidence:` line: a CI run, test name or recorded session that fails on the base commit and passes on the PR head.
- Security, persistence and safety packages get a second Claude (Opus) review.
- Workers are `beam:cloud`, escalating to `kimi-k3:cloud`, with Claude subagents as fallback. No other model reviews or writes code.

## Out of scope

Rewriting main's history. The plan fixes forward; the unsquashed history is kept at tag `pre-squash-history`.
