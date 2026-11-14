# archives: OAR and IAR archive support

## Summary

Port the OAR (OpenSim Archive) and IAR (Inventory Archive) systems from
OpenSimulator 0.9.3.0, covering ~8,200 lines of C# across 23 files. OAR
archives save and restore entire regions (objects, terrain, parcels, settings,
assets). IAR archives save and restore user inventory trees with assets.

Both formats are gzip-compressed tar files with a well-defined directory
layout. The custom `TarArchiveWriter`/`TarArchiveReader` (455 lines) are
replaced by the `tar` and `flate2` crates. Cross-grid portability requires
`OspResolver` for UUID-to-name mapping and `ExternalRepresentationUtils` for
creator/owner UUID rewriting.

Reference: `OpenSim/Region/CoreModules/World/Archiver/` (15 files, ~4,800
lines) and `OpenSim/Region/CoreModules/Avatar/Inventory/Archiver/` (8 files,
~3,400 lines).

## Scope

### In scope

- OAR write: single-region and multi-region archives with async asset
  gathering, timeout handling, and progress callbacks.
- OAR read: full dearchive with merge mode, displacement vector, rotation,
  bounding-box filter, permission filter, and default-user substitution.
- IAR write: inventory folder-tree walk with permission filtering and asset
  gathering.
- IAR read: folder creation, item insertion, creator UUID resolution via
  OspResolver.
- Region settings serializer (`RegionSettingsSerializer`, 368 lines).
- Land data serializer (`LandDataSerializer`, 256 lines).
- `OspResolver`: first.last name to UUID mapping for portable archives.
- `ExternalRepresentationUtils`: rewrite creator/owner/lastOwner UUIDs on
  dearchive for cross-grid compatibility.
- Console commands: `save oar`, `load oar`, `save iar`, `load iar` with all
  C# options.
- Port tests from `ArchiverTests.cs` and `InventoryArchive*Tests.cs` (~900
  lines).

### Out of scope

- Streaming OAR to S3 or object storage (future enhancement).
- OAR format changes or extensions beyond C# 0.9.3.0 compatibility.
- HG inventory transfer (covered by `hypergrid` change).

## Dependencies

All required modules are already ported:

- Scene graph and XML2 serializer (`region-simulator-core`)
- Asset service (`grid-services`)
- Inventory service (`grid-services`)
- Terrain heightfield (`region-simulator-core`)
- Land/parcel data (`region-modules` section 2)
- Permissions checks (`region-modules` section 3)
- UserManagement UUID resolution (`region-modules` section 12)

## Crates

- `tar` (crates.io, MIT/Apache-2.0): tar archive read/write.
- `flate2` (already in workspace): gzip compression/decompression.
- Reuse existing `opensim-types`, `opensim-services`, `opensim-region` crates.

New module: `opensim-archives` crate, or a `archives` module within
`opensim-region` depending on size.

## Design

### OAR structure

```
archive.xml                     # version, timestamps, region dimensions
assets/{uuid}_{type}.{ext}      # raw asset blobs (21 type mappings)
objects/{name}_{x}-{y}-{z}__{uuid}.xml  # XML2-serialized SceneObjectGroups
terrains/{region}_{x}-{y}.r32   # raw f32 heightfield
settings/{region}.xml           # region settings
landdata/{parcel-uuid}.xml      # parcel definitions
regions/                        # multi-region: one subdirectory per region
```

### IAR structure

```
archive.xml
inventory/{FolderName}__{uuid}/{ItemName}__{uuid}.xml
assets/{uuid}_{type}.{ext}
userprofiles/{uuid}.xml         # optional OspResolver data
```

### Key design decisions

- A1: Use `tar` + `flate2` instead of porting the custom tar implementation.
  The C# tar code is minimal and single-purpose; the Rust `tar` crate handles
  edge cases (long paths, symlinks, permissions) correctly.
- A2: Async asset gathering with `tokio::time::timeout` replacing the C#
  `ManualResetEvent` + timer pattern.
- A3: OAR load options (merge, displace, rotate, bbox) are a flags struct,
  not positional arguments, matching the C# `Dictionary<string, object>`.
- A4: OspResolver writes `first.last` profile entries during OAR save and
  resolves them during load. For grids without profile data, fall back to
  UserManagement service.
- A5: Path traversal protection on tar extraction — all paths validated
  against the expected prefix before write.

## Security considerations

- **Path traversal** (HIGH): tar entries with `../` or absolute paths must be
  rejected during extraction. The `tar` crate provides `Entry::unpack_in`
  with built-in protection, but manual path validation is added as defense in
  depth.
- **Asset size limits**: enforce maximum individual asset size and total
  archive size during dearchive to prevent memory exhaustion.
- **UUID injection**: OspResolver name→UUID mappings from untrusted archives
  must not overwrite existing grid users. Validate against the user service.
- **Permission checks**: OAR save requires estate owner or explicit export
  permission. IAR save respects per-item transfer permissions.
