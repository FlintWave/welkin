<!-- SPDX-License-Identifier: LGPL-3.0-or-later -->
# OpenSpec workflow

[← Back to docs](index.md) · [Task system](task-system.md) · [Crate structure](crate-structure.md)

Welkin uses OpenSpec, a spec-driven planning tool with a command line client, to plan work.
Each piece of work is a change with a written proposal, a design and a task list,
so a person or an agent can pick it up with no other context.

## Layout

```text
openspec/
  config.yaml          schema: spec-driven
  specs/               the standing specifications
  changes/
    <change-name>/
      proposal.md      what and why, and what is out of scope
      design.md        scope, architecture and decisions
      tasks.md         the checklist (see the task system)
      specs/<area>/spec.md   spec changes this work makes
    archive/
      2026-11-15-<change-name>/   finished changes, date-prefixed
```

## The cycle

1. **Propose.** Create `openspec/changes/<name>/` with a proposal, a design and a
   task list. Keep the scope small enough to finish.
2. **Branch.** Make a feature branch from `dev`.
3. **Apply.** Work through `tasks.md` in order. Tick a task only when it is done,
   and put the commit hash beside it.
4. **Review.** Open a pull request to `dev`. CI must pass. Changes to security,
   permissions, persistence or encryption get a second review.
5. **Archive.** When every task is done and verified, archive the change so its
   spec changes fold into `openspec/specs/` and the folder moves under
   `changes/archive/`.

## Commands

The `openspec` command line tool works on this layout:

| Command | Use |
|---|---|
| `openspec list` | List the active changes (`--specs` lists specs) |
| `openspec view` | Interactive dashboard of specs and changes |
| `openspec change` | Manage change proposals |
| `openspec spec` | View specifications |
| `openspec archive <name>` | Archive a completed change and update the specs |

## Rules

- A change is not done because its code exists. It is done when the behaviour is
  wired into a running binary and checked, ideally by a test that exercises the
  real path.
- Disk is the source of truth. Count tasks with `grep`, not from memory (see the
  [task system](task-system.md)).
- If a design turns out wrong, change `design.md` and say why, instead of leaving
  the code and the design in disagreement.
