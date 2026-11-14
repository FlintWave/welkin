## 1. Foundation

- [x] 1.1 Archive constants module: 21 asset-type-to-extension mappings, tar path prefixes, control file version, max archive size constant (5207afc: constants.rs, 9 tests)
- [x] 1.2 RegionSettingsSerializer: XML read/write for region settings matching C# format, round-trip test against C# output (5207afc: region_settings.rs, 4 tests)

## 2. Land and settings serialization

- [x] 2.1 LandDataSerializer: XML read/write for parcel data matching C# format, round-trip test (5207afc: land_data.rs, 4 tests)
- [x] 2.2 Archive control file (archive.xml): version, timestamps, region dimensions, multi-region flag (5207afc: control_file.rs, 5 tests)

## 3. OAR write

- [x] 3.1 OAR write (single region): gather scene objects, terrain, land, settings; stream to tar.gz via `tar` + `flate2`; progress callback (4853ed9: oar_write.rs, 3 tests)
- [x] 3.2 Async asset gathering: walk object asset references, fetch from asset service with configurable timeout, deduplicate (4853ed9: assets_request.rs, 7 tests)
- [x] 3.3 OAR write (multi-region): ArchiveScenesGroup, per-region subdirectories, cross-region asset dedup (4853ed9: oar_write.rs, 2 tests)

## 4. OAR read

- [x] 4.1 OAR read (single region, basic): parse tar entries, dearchive objects via XML2 deserializer, load terrain and parcels, restore settings (4853ed9: oar_read.rs, 12 tests)
- [x] 4.2 OAR read options: merge mode (skip terrain/parcels), displacement vector, rotation, bounding-box filter, default-user substitution (4853ed9: oar_read.rs, 7 tests)
- [x] 4.3 OAR permission filter: check export permissions during save, check import permissions during load, no-assets mode (4853ed9: oar_write.rs + oar_read.rs, 4 tests)

## 5. IAR

- [x] 5.1 IAR write: inventory folder-tree walk, item serialization (UserInventoryItemSerializer), permission filter, asset gathering (21945b2: iar_write.rs, 18 tests)
- [x] 5.2 IAR read: folder creation, item insertion, creator UUID resolution, handle missing assets gracefully (21945b2: iar_read.rs, 11 tests)

## 6. Cross-grid portability

- [x] 6.1 OspResolver: first.last to UUID mapping, write profiles during save, resolve during load, fallback to UserManagement service (21945b2: osp_resolver.rs, 12 tests)
- [x] 6.2 ExternalRepresentationUtils: rewrite creator/owner/lastOwner UUIDs on dearchive, handle HG-format UUIDs (uuid;uri) (21945b2: external_representation.rs, 17 tests)

## 7. Integration

- [ ] 7.1 ArchiverModule region module: console commands (save oar, load oar with all options), integration with module registry
- [ ] 7.2 InventoryArchiverModule: console commands (save iar, load iar), user validation, progress reporting

## 8. Testing and security

- [ ] 8.1 Port ArchiverTests and InventoryArchiveTests (~900 lines C#): round-trip save/load, merge, displacement, permission filter, multi-region
- [ ] 8.2 SECURITY REVIEW: path traversal on tar extraction, asset size limits, UUID injection via OspResolver, permission bypass (HIGH RISK, Opus reviewer)
