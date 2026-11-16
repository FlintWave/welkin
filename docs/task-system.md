<!-- SPDX-License-Identifier: LGPL-3.0-or-later -->
# Task system

[← Back to docs](index.md) · [OpenSpec workflow](openspec-workflow.md) · [Testing](testing.md)

Work is tracked in `tasks.md` files, one per change under `openspec/changes/`.
The files are plain Markdown checklists, so people and agents read and update
them the same way.

## Format

```markdown
## 1. Foundation

- [x] 1.1 Archive constants module: tar path prefixes, control file version (5207afc: constants.rs, 9 tests)
- [ ] 1.2 RegionSettingsSerializer: XML read and write, round-trip test
```

- `- [ ]` is open and `- [x]` is done.
- A done task names the commit that finished it, and says what proves it (the
  file, the number of tests).
- Group tasks under numbered headings. Order them so each task can start once
  the ones above it are done.
- Write each task so one person can finish it in one sitting, and so it says what
  "done" means.

## Counting

The counts on disk are the truth. From the repository root:

```sh
grep -c '\[x\]' openspec/changes/*/tasks.md
grep -c '\[ \]' openspec/changes/*/tasks.md
```

Do not trust a remembered total or an agent's report of "done". Check the file,
check the commit, and run the test.

## The roadmap

[ROADMAP.md](https://github.com/FlintWave/welkin/blob/main/ROADMAP.md) summarises
the state of all changes: what is in the current release, what is paused, and
which gaps are known. Update it when a change finishes or a gap closes, and take
the numbers from the `tasks.md` files.

## Working with agents

When you hand a task to an AI agent:

- Give it the change folder and the task number, not a summary.
- Tell it what the output must be and which files it may touch.
- Review what it returns against the code and the tests. An agent's report of what
  it did describes what it meant to do, not necessarily what it did.
- Keep the rules above: real work in every handler, wired into a binary, checked
  by a test, and no stubs (CI fails on them).
