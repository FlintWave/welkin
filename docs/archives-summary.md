# Archives summary

Status: accepted (18 of 18 tasks, 8 sections, 205 tests). Security review
verdict: CONDITIONAL PASS. Change: `openspec/changes/archives/`.

## What was built

The OAR (OpenSim Archive) and IAR (Inventory Archive) systems from
`OpenSim/Region/CoreModules/World/Archiver/` (15 files, ~4,800 lines) and
`OpenSim/Region/CoreModules/Avatar/Inventory/Archiver/` (8 files, ~3,400
lines), in one crate: `opensim-archive` (9,555 lines, 205 tests).

| Section | Area | Tasks |
|---|---|---|
| 1 | Foundation: constants, 21 asset-type mappings, path prefixes | 1.1-1.2 |
| 2 | Land and settings serialization, archive control file | 2.1-2.2 |
| 3 | OAR write: single/multi-region, async asset gathering | 3.1-3.3 |
| 4 | OAR read: merge, displace, rotate, bbox, permissions | 4.1-4.3 |
| 5 | IAR write and read | 5.1-5.2 |
| 6 | Cross-grid portability: OspResolver, ExternalRepresentationUtils | 6.1-6.2 |
| 7 | Integration: ArchiverModule, InventoryArchiverModule, console commands | 7.1-7.2 |
| 8 | Ported tests and security review | 8.1-8.2 |

## Architecture

Both OAR and IAR are gzip-compressed tar files. The C# custom
`TarArchiveWriter`/`TarArchiveReader` (455 lines) is replaced by the `tar` and
`flate2` crates.

| Module | Responsibility |
|---|---|
| `constants.rs` | 21 asset-type-to-extension mappings, path prefixes, filename helpers |
| `region_settings.rs` | `RegionSettingsSerializer`: XML read/write matching C# format |
| `land_data.rs` | `LandDataSerializer`: XML read/write for parcel data |
| `control_file.rs` | `archive.xml` parsing: version, timestamps, region dimensions, multi-region |
| `oar_write.rs` | Single and multi-region OAR save with progress callbacks |
| `oar_read.rs` | Full dearchive with merge, displacement, rotation, bbox, permission filter |
| `assets_request.rs` | Async asset gathering with `tokio::time::timeout`, deduplication |
| `iar_write.rs` | Inventory tree walk, permission filter, `UserInventoryItemSerializer` |
| `iar_read.rs` | Folder creation, item insertion, creator UUID resolution |
| `osp_resolver.rs` | `first.last` name-to-UUID mapping for portable archives |
| `external_representation.rs` | Creator/owner/lastOwner UUID rewriting for cross-grid compatibility |
| `archiver_module.rs` | Region module: `save oar`, `load oar` console commands |
| `inventory_archiver_module.rs` | Region module: `save iar`, `load iar` console commands |

## Key decisions

- A1: Use `tar` + `flate2` instead of porting the 455-line custom tar code.
- A2: Async asset gathering with `tokio::time::timeout` replaces C#
  `ManualResetEvent` + timer.
- A3: OAR load options are a flags struct, not positional arguments.
- A4: OspResolver writes `first.last` profiles on save, resolves on load, falls
  back to UserManagement service.
- A5: Path traversal protection: all tar paths validated against expected prefix
  before processing.

## Tests

174 unit tests and 31 integration tests (205 total). Integration tests
round-trip OAR and IAR save/load, testing merge, displacement, permission
filter, and multi-region support. Ported from `ArchiverTests.cs` and
`InventoryArchive*Tests.cs` (~900 lines C#).

## Security review

Verdict: **CONDITIONAL PASS** (0 critical, 0 high, 5 medium). Full report:
`docs/security-review-archives.md`.

| ID | Severity | Finding |
|---|---|---|
| M-1 | MEDIUM | Decompression bomb: no limit on total decompressed size |
| M-2 | MEDIUM | `Vec::with_capacity` trusts tar header size (up to 50 GiB) |
| M-3 | MEDIUM | `from_utf8_lossy` on tar paths can silently alter path validation |
| M-4 | MEDIUM | `validate_path` does not check for null bytes (OAR reader) |
| M-5 | MEDIUM | `validate_path` does not check for Windows drive-letter paths |

All 5 are defense-in-depth gaps tracked for remediation before production.

## Commits

| Hash | Description |
|---|---|
| `5207afc` | Sections 1-2: constants, serializers, control file (22 tests) |
| `4853ed9` | Sections 3-4: OAR write/read, async assets, multi-region (35 tests) |
| `21945b2` | Sections 5-6: IAR, OspResolver, ExternalRepresentationUtils (58 tests) |
| `62994dd` | Sections 7-8.1: integration modules, ported tests (90 tests) |
| `0e1b978` | Section 8.2: security review, CONDITIONAL PASS, accepted |
