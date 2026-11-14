## 1. Decisions

- [x] 1.1 D1: operator decides the project licence (GPL-3.0-or-later, 2026-11-13)
- [ ] 1.2 D2: operator decides repository name and home

## 2. Audit

- [ ] 2.1 List every file in `ThirdPartyLicenses/` and every binary in `bin/` with licence and disposition
- [ ] 2.2 Establish the licence of the protocol message template
- [ ] 2.3 Establish the licence of `bin/openmetaverse_data` and each `bin/assets` set
- [ ] 2.4 Check Warp3D and remaining DLLs not yet classified
- [ ] 2.5 Evaluate JPEG 2000: `aurora-assets` (uses `openjpeg-sys`) against a pure Rust decoder
- [ ] 2.6 Read the `[patch.crates-io]` section of Aurora's workspace and list what a git dependency would not inherit
- [x] 2.7 Copy the relevant `NOTICE.md` entries for each reused Aurora crate

## 3. Outputs

- [ ] 3.1 Write `docs/licence-audit.md`
- [ ] 3.2 Write the `cargo deny` allow-list
- [ ] 3.3 Write naming and trademark rules
