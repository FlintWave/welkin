# Security Review: opensim-archive crate

Date: 2026-11-14
Reviewer: Claude Opus (adversarial review)
Verdict: CONDITIONAL PASS

## Summary

The `opensim-archive` crate handles untrusted OAR/IAR archive files from users and other grids. The crate demonstrates good security awareness: path traversal validation is present on both OAR and IAR read paths, size limits are enforced, item IDs are reassigned on import to prevent collisions, and OSPA resolution is read-only (never creates user mappings). However, several defense-in-depth gaps exist that should be addressed before production use, most notably around decompression bombs, `from_utf8_lossy` path bypasses, unbounded collection growth, and the `Vec::with_capacity` trust of tar headers.

## Findings

### M-1: Decompression bomb -- no limit on total decompressed size
**Severity:** MEDIUM
**File:** `oar_read.rs:226-294` and `oar_read.rs:300-395`
**Description:** The OAR reader wraps the input in `GzDecoder` and iterates tar entries. While individual entry sizes are validated against `MAX_ARCHIVE_SIZE` (50 GiB), there is no limit on the total decompressed output. A crafted archive could contain thousands of entries each just under 50 GiB, or a gzip stream with an extreme compression ratio. The `flate2::read::GzDecoder` will decompress the stream without any overall size accounting.
**Impact:** An attacker could craft a small gzip file (a few KB) that decompresses to gigabytes of data, exhausting server memory and causing denial of service.
**Recommendation:** Track cumulative bytes read across all entries and abort when a configurable total limit (e.g. `MAX_TOTAL_DECOMPRESSED_SIZE`) is exceeded. Consider also limiting the number of entries (e.g. 500,000) to prevent entry-count bombs.
**Status:** OPEN

### M-2: `Vec::with_capacity` trusts tar header size before reading
**Severity:** MEDIUM
**File:** `oar_read.rs:254` and `oar_read.rs:327`
**Description:** The code does `Vec::with_capacity(entry_size as usize)` using the size declared in the tar header. Although the size is validated against `MAX_ARCHIVE_SIZE` (50 GiB), that constant is extremely large -- up to 50 GiB could be pre-allocated in a single `Vec`. On a 64-bit system the `as usize` cast of a `u64` up to `50 * 1024^3` will not overflow, but the allocation itself could exhaust memory. Additionally, the tar header size could lie: the `read_to_end` call will read the actual decompressed data, which could differ from the declared size.
**Impact:** A malicious archive declaring `entry_size = 50_000_000_000` would attempt to allocate ~50 GiB, likely causing an OOM kill. Even with the size check passing, the allocation is too large in practice.
**Recommendation:** Cap the per-entry allocation at a more realistic limit for each entry type. For example: object XML < 64 MiB, terrain < 64 MiB, settings < 1 MiB, assets < 256 MiB. The IAR reader already does this (separate `MAX_ASSET_SIZE` and `MAX_ITEM_XML_SIZE` constants). Apply the same discipline to OAR entries. Also consider using `read_to_end` on a limited reader (`io::Read::take`) rather than pre-allocating from the header.
**Status:** OPEN

### M-3: `from_utf8_lossy` on tar paths can silently alter path validation
**Severity:** MEDIUM
**File:** `oar_read.rs:239` and `oar_read.rs:316`
**Description:** The OAR reader uses `String::from_utf8_lossy(&path_bytes)` to convert tar entry paths. This replaces invalid UTF-8 sequences with the Unicode replacement character (U+FFFD). A malicious tar entry could encode `..` using a sequence that, after lossy conversion, produces `..` or produces a path that passes `validate_path` but is interpreted differently by downstream code. For example, an overlong UTF-8 encoding of `/` (0xC0 0xAF) would be replaced by `\u{FFFD}\u{FFFD}`, which is benign -- but other encoding tricks might interact differently with path operations.
**Impact:** Theoretical path traversal bypass via encoding manipulation. The current `validate_path` checks on the lossy-converted string, and the paths are only used as map keys and prefix-matches within the archive reader (not for filesystem writes), so the risk is currently contained. However, if a `SceneImporter` implementation uses these paths for filesystem operations, the risk increases.
**Recommendation:** Reject non-UTF-8 paths entirely rather than silently converting them. Replace `from_utf8_lossy` with `from_utf8` and return an error on invalid UTF-8. The IAR reader correctly uses `std::str::from_utf8` in `load_control_file` but relies on the caller to pass a `&str`.
**Status:** OPEN

### M-4: OAR `validate_path` does not check for null bytes
**Severity:** MEDIUM
**File:** `oar_read.rs:146-171`
**Description:** The OAR reader's `validate_path` function checks for `..` components and absolute paths but does not check for null bytes (`\0`). In contrast, the IAR reader's `validate_tar_path` (iar_read.rs:79-101) correctly rejects null bytes. Null bytes in paths can truncate strings when passed to C APIs (e.g., filesystem operations in FFI) and may bypass path validation.
**Impact:** If a `SceneImporter` passes the path to any C-backed filesystem operation, a null byte could truncate the path and cause writes to unintended locations. Within the archive reader itself, which only uses paths for string matching, the risk is limited.
**Recommendation:** Add a null byte check to `validate_path` in `oar_read.rs` to match the IAR reader's `validate_tar_path`.
**Status:** OPEN

### M-5: OAR `validate_path` does not check for backslash-separated absolute paths with drive letters
**Severity:** MEDIUM
**File:** `oar_read.rs:146-171`
**Description:** The function checks `path.starts_with('\\')` for Windows-style absolute paths, but does not check for drive-letter paths like `C:\Windows\System32`. While the archive reader does not perform filesystem writes, a downstream `SceneImporter` might use these paths on Windows.
**Impact:** A path like `C:\important\file` would pass `validate_path` and could be used by a careless `SceneImporter` implementation for an absolute filesystem write on Windows.
**Recommendation:** Add a check for `path.len() >= 2 && path.as_bytes()[1] == b':'` to reject drive-letter paths. More robustly, consider maintaining a single path validation function shared between OAR and IAR readers.
**Status:** OPEN

### L-1: `check_import_permission` is a no-op
**Severity:** LOW
**File:** `oar_read.rs:447-455`
**Description:** The `check_import_permission` function always returns `true` regardless of inputs: `checker.is_none() || true`. The `|| true` short-circuits any permission check. The comment says "Import permission is always granted in the current C# code" which matches the upstream behavior, but this means the function provides a false sense of security to callers.
**Impact:** Any object in an archive will be imported regardless of permissions. An attacker could craft an archive with objects that should be blocked by permission checks (e.g., no-copy objects) and they would be imported anyway.
**Recommendation:** Either implement actual permission checking or remove the function entirely to avoid giving callers false confidence. If keeping it as a stub, change the return type or add a `todo!()` so it is obviously unimplemented.
**Status:** ACCEPTED (skeleton phase -- this matches C# behavior and will be revisited during full permission implementation)

### L-2: Unbounded growth of collections during archive load
**Severity:** LOW
**File:** `oar_read.rs:232` (assets Vec), `oar_read.rs:280` (regions HashMap), `iar_read.rs:139-146` (resolved_folders, item_id_map, deferred_links, creator_for_asset)
**Description:** During archive loading, multiple unbounded collections grow with no limit on the number of entries. An archive with millions of tiny entries could cause excessive memory usage even if each individual entry is within size limits.
**Impact:** Denial of service through memory exhaustion. While the per-entry size is checked, the number of entries is not.
**Recommendation:** Add configurable limits on: number of assets, number of objects per region, number of regions, number of IAR items, and number of IAR folders. Reject archives that exceed these limits.
**Status:** OPEN

### L-3: OspResolver cache grows without bound
**Severity:** LOW
**File:** `osp_resolver.rs:129-160`
**Description:** The `OspResolverCache` stores every OSPA lookup result in a `HashMap` with no size limit. A malicious archive could contain millions of unique OSPA strings (e.g., `ospa:n=Name1 Surname1`, `ospa:n=Name2 Surname2`, ...), each requiring a cache entry.
**Impact:** Memory exhaustion during IAR load if the archive contains many unique OSPA creator references.
**Recommendation:** Add a maximum cache size (e.g., 10,000 entries) with LRU eviction, or limit the number of unique creator references processed.
**Status:** OPEN

### L-4: UUID rewriting via `RewriteContext` has no access control
**Severity:** LOW
**File:** `external_representation.rs:82-122`
**Description:** The `RewriteContext` mapping is populated by the caller with no validation. Any UUID can be mapped to any other UUID. If the mapping is populated from untrusted data (e.g., a `userprofiles/` section in an archive), an attacker could map any creator/owner UUID to any other, including admin or system UUIDs.
**Impact:** If a caller populates the `RewriteContext` from archive data without validation, objects could be assigned to arbitrary users, including grid administrators.
**Recommendation:** The `RewriteContext` is a passive data structure; the security responsibility lies with callers. Document clearly that callers MUST NOT populate the mapping from untrusted archive data without validating each mapping against the grid's user service. Consider adding a `validate_mapping` method that checks each target UUID exists in the grid.
**Status:** OPEN

### L-5: OAR `MAX_ARCHIVE_SIZE` is 50 GiB -- too large for practical DoS prevention
**Severity:** LOW
**File:** `constants.rs:48`
**Description:** The `MAX_ARCHIVE_SIZE` constant is set to `50 * 1024 * 1024 * 1024` (50 GiB). This is the per-entry size limit for OAR archives. While technically correct as an upper bound, it is far too large to prevent practical DoS attacks. A single 50 GiB entry would exhaust memory on most servers.
**Impact:** The size check passes for entries up to 50 GiB, allowing a single entry to allocate far more memory than most servers have available.
**Recommendation:** Reduce `MAX_ARCHIVE_SIZE` to a more realistic value (e.g., 2 GiB for the total archive, with per-entry-type limits). The IAR reader's separate `MAX_ASSET_SIZE` (256 MiB) and `MAX_ITEM_XML_SIZE` (16 MiB) are better calibrated.
**Status:** OPEN

### I-1: XXE attacks not a risk with `quick-xml`
**Severity:** INFO
**File:** All XML parsing (control_file.rs, region_settings.rs, land_data.rs, iar_write.rs)
**Description:** The `quick-xml` crate (v0.37) is a streaming pull parser that does not implement DTD processing or external entity resolution. It does not support `<!DOCTYPE>`, `<!ENTITY>`, or entity expansion beyond the five predefined XML entities (`&lt;`, `&gt;`, `&amp;`, `&quot;`, `&apos;`). This means XXE (XML External Entity) attacks are not possible through this parser.
**Impact:** None. This is a positive finding.
**Recommendation:** No action needed. Document that XXE safety depends on using `quick-xml` and would need re-evaluation if the XML library changes.
**Status:** MITIGATED

### I-2: Terrain size is not validated against expected dimensions
**Severity:** INFO
**File:** `oar_read.rs:484-516` (process_region_entry), `oar_write.rs:153` (OarTerrain)
**Description:** The terrain heightfield data is stored as raw bytes and not validated against the expected region size. For a 256x256 region, the terrain should be exactly 262,144 bytes (256 * 256 * 4 bytes per f32). The code does not verify this. Integer overflow is not a concern here because the multiplication `256 * 256 * 4 = 262144` is well within `usize` range, and the region sizes come from the control file (parsed as `u32`). However, a malicious archive could provide terrain data of the wrong size, which could cause issues in downstream terrain loading code (e.g., out-of-bounds reads if the terrain is too short).
**Impact:** Potential panic or incorrect terrain rendering if terrain data size does not match expected dimensions.
**Recommendation:** Add a check that `terrain.len() == (size_x * size_y * 4)` before passing terrain data to the importer.
**Status:** OPEN

### I-3: IAR reader reassigns item IDs -- good security practice
**Severity:** INFO
**File:** `iar_read.rs:319-320`
**Description:** The IAR reader generates new UUIDs for imported items (`let new_id = Uuid::new_v4()`) rather than reusing the IDs from the archive. This prevents an attacker from crafting an archive that overwrites specific existing inventory items by ID.
**Impact:** None. This is a positive finding.
**Recommendation:** No action needed.
**Status:** MITIGATED

### I-4: OSPA resolution is read-only -- UUID injection prevented by design
**Severity:** INFO
**File:** `osp_resolver.rs:109-119`, `iar_read.rs:327-337`
**Description:** The `resolve_ospa_name` function only performs lookups against the user service -- it never creates new user accounts or mappings. If an OSPA name does not match an existing user, the resolution returns `None` and the IAR reader falls back to the importing user's UUID. The `user_exists` method on `OspResolverCache` provides an additional guard that callers can use before creating mappings. This design prevents an attacker from injecting fake user profiles via crafted OSPA strings.
**Impact:** None. This is a positive finding.
**Recommendation:** Document this security property in the module-level documentation. Ensure that any future code that creates user mappings from OSPA data always calls `user_exists` first.
**Status:** MITIGATED

### I-5: Wipe-owners mode correctly zeroes sensitive IDs
**Severity:** INFO
**File:** `oar_write.rs:504-509`, `land_data.rs:147,168,172,186`
**Description:** When `wipe_owners` is true, the land data serializer correctly sets `OwnerID`, `GroupID`, and `IsGroupOwned` to nil/false values. This prevents accidental leakage of user identity when publishing archives.
**Impact:** None. This is a positive finding.
**Recommendation:** No action needed. Consider extending wipe-owners to also clear `AuthBuyerID` and access list entries for full anonymization.
**Status:** MITIGATED

## Untrusted Data Flow Analysis

Tracing the flow of untrusted data from archive bytes:

1. **Archive bytes -> GzDecoder -> tar entries**: Decompression is unbounded (M-1). Individual entry sizes are checked but the limit is too generous (L-5, M-2).

2. **Tar entry path -> String -> validate_path/validate_tar_path -> route_path**: Path validation is present but has gaps: no null byte check in OAR reader (M-4), `from_utf8_lossy` could mask encoding attacks (M-3), no Windows drive-letter check (M-5). Paths are NOT used for filesystem writes within this crate -- they are only used for string matching and routing. The IAR reader's validation is more complete.

3. **Tar entry data -> XML parser -> structs**: XML parsing uses `quick-xml` which is XXE-safe (I-1). Parsed values (UUIDs, numbers, strings) are validated at parse time. Unknown elements are silently ignored.

4. **Tar entry data -> Vec<u8> -> asset store**: Asset data flows through to the `SceneImporter.store_asset` or `InventoryImporter.store_asset` trait methods. Size limits are enforced for IAR assets (256 MiB) but not effectively for OAR assets (50 GiB limit is too high).

5. **OSPA strings -> OspResolver -> UUID**: Resolution is read-only; cannot create users (I-4). Cache growth is unbounded (L-3).

6. **RewriteContext -> XML string replacement**: UUID rewriting is a passive find-and-replace. No filesystem operations. The security risk is in how callers populate the mapping (L-4).

7. **Object XML -> SceneImporter.create_object**: Raw XML is passed to the importer. The crate does not validate object XML beyond basic deserialization. The importer is responsible for validating object properties, permissions, and script safety.

## Verdict Rationale

CONDITIONAL PASS because:
- **No CRITICAL findings**: No directly exploitable RCE or data loss paths were identified.
- **Path traversal is handled**: Both OAR and IAR readers validate paths, and the crate does not perform filesystem writes.
- **XXE is not possible**: The XML parser does not support external entities.
- **UUID injection is prevented**: OSPA resolution is read-only, and item IDs are reassigned on import.
- **Conditions for PASS**: Fix M-1 (decompression bomb limit), M-2 (allocation from header), M-3 (reject non-UTF-8 paths), and M-4 (null byte check in OAR). These are defense-in-depth gaps that become exploitable if downstream code (SceneImporter implementations) is less careful.
