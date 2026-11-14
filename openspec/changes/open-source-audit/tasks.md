## 1. Decisions

- [x] 1.1 D1: operator decides the project licence (GPL-3.0-or-later, 2026-11-13)
- [x] 1.2 D2: operator decides repository name and home (opensim-rust, local-only for now; public home TBD pending trademark audit)

## 2. Audit

- [x] 2.1 List every file in `ThirdPartyLicenses/` and every binary in `bin/` with licence and disposition
- [x] 2.2 Establish the licence of the protocol message template
- [x] 2.3 Establish the licence of `bin/openmetaverse_data` and each `bin/assets` set
- [x] 2.4 Check Warp3D and remaining DLLs not yet classified
- [x] 2.5 Evaluate JPEG 2000: `aurora-assets` (uses `openjpeg-sys`) against a pure Rust decoder
- [x] 2.6 Read the `[patch.crates-io]` section of Aurora's workspace and list what a git dependency would not inherit
- [x] 2.7 Copy the relevant `NOTICE.md` entries for each reused Aurora crate

## 3. Outputs

- [x] 3.1 Write `docs/licence-audit.md` (b3518a5: executive summary, recommendations, full audit)
- [x] 3.2 Write the `cargo deny` allow-list (b3518a5: GPL-3.0-or-later fix, missing licences, advisory updates)
- [x] 3.3 Write naming and trademark rules (b3518a5: docs/naming-and-trademarks.md)
