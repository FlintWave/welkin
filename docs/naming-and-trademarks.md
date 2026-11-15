# Naming and Trademark Guidelines

[← Back to docs](README.md) · [Licence audit](licence-audit.md)

This document covers trademark obligations and naming conventions for
Welkin.

## Trademarks we reference

**OpenSimulator** is a trademark of the OpenSimulator project
(opensimulator.org). Welkin is derived from OpenSimulator source code
but is a separate project. We do not claim endorsement by or affiliation
with the OpenSimulator project.

**Second Life** is a registered trademark of Linden Research, Inc. Use
it only when referring to the Second Life platform or protocol in
factual context (e.g., "implements the Second Life protocol"). Never
imply endorsement by or partnership with Linden Research.

**Aurora Viewer** and the Aurora logo are trademarks of their
respective owners. Welkin depends on Aurora Viewer crates as build
dependencies but does not use the Aurora name or branding in its own
identity.

## Our project name

The project name is **Welkin** (Old English for "the sky, the firmament").
Crate names use the `opensim-` prefix as a compatibility signal (e.g.,
`opensim-services`, `opensim-scene`). Binary names: `opensim-robust`
(grid services), `opensim-region` (region simulator).

## Contributor guidelines

1. **Use "OpenSimulator"** (full name) when referring to the upstream
   C# project in documentation, comments, and commit messages. Do not
   shorten to "OpenSim" in public-facing text -- the abbreviation is
   widely used in the community but may carry trademark implications.

2. **The `opensim-` prefix in crate and binary names** is acceptable as
   a compatibility signal. Do not use "OpenSimulator" as a standalone
   product name for this project.

3. **Do not use the OpenSimulator logo** or any visual branding from
   the upstream project.

4. **Do not use "Second Life"** in crate names, binary names, or
   marketing material. In code comments and documentation, use it only
   for factual protocol references.

5. **Do not use the Aurora Viewer name or logo** in any user-facing
   context. The Aurora dependency is an implementation detail. Refer to
   it as "Aurora Viewer crates" in technical documentation only.

6. **LSL** (Linden Scripting Language) is a descriptive term used by
   multiple virtual-world projects. No trademark issue has been
   identified. Use freely in technical context.

## Summary

| Name | Owner | Our use |
|---|---|---|
| OpenSimulator | OpenSimulator project | Descriptive reference; no affiliation claim |
| OpenSim | OpenSimulator project (informal) | Avoid in public text; use full name |
| Welkin | This project | Project name |
| opensim-* crates | This project | Crate names (compatibility prefix) |
| opensim-robust, opensim-region | This project | Binary names |
| Second Life | Linden Research, Inc. | Factual protocol references only |
| Aurora Viewer | Aurora Viewer project | Technical docs only; not in branding |
| LSL | Descriptive term | Free use in technical context |
