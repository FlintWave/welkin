# Security Review: Remote Write Paths (External-Grid Mode 3.5)

**Date:** 2026-11-13
**Scope:** Every code path that writes to, or sends data towards, a remote
grid's services (assets, inventory, grid registration, neighbour notification).
**Threat model:** The remote grid is assumed hostile.

---

## Findings

### CRITICAL-1: SSRF via neighbour `server_uri`

**File:** `crates/opensim-services/src/registration.rs`, lines 96-103
**Severity:** CRITICAL

The `notify_neighbours()` method constructs URLs from `server_uri` values
returned by the remote grid service:

```rust
let url = format!(
    "{}/region/{}/",
    nbr.server_uri.trim_end_matches('/'),
    self.region.uuid
);
self.client.post(&url).json(&body).send().await
```

A malicious grid can return arbitrary URIs in the neighbour list
(`get_neighbours` response), causing the region server to issue HTTP POST
requests to attacker-controlled or internal endpoints:

- `http://169.254.169.254/latest/meta-data/` (cloud instance metadata)
- `http://localhost:8080/admin/delete-all` (internal services)
- Any internal network address

This is a textbook Server-Side Request Forgery (SSRF).

**Recommended fix:** Validate every neighbour `server_uri` before using it:
1. Parse as a URL; reject non-HTTP(S) schemes.
2. Resolve the hostname and reject private/link-local/loopback IP ranges
   (RFC 1918, RFC 4193, 169.254.0.0/16, ::1, etc.).
3. Optionally maintain an allowlist of known grid address patterns.

---

### HIGH-1: XML injection in `asset_exists`

**File:** `crates/opensim-services/src/remote/asset.rs`, lines 94-101
**Severity:** HIGH

The `asset_exists` method interpolates the `id` parameter directly into an
XML document without escaping:

```rust
let xml = format!(
    "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
     <ArrayOfString ...>\
     <string>{id}</string>\
     </ArrayOfString>"
);
```

If `id` contains XML metacharacters (e.g.
`</string><string>TARGET-UUID</string><string>`), an attacker who controls
the queried ID can inject additional `<string>` elements, probing for the
existence of arbitrary asset UUIDs on the remote grid.

**Recommended fix:** Use `quick_xml::escape::escape(&id)` before
interpolation, consistent with how `write_asset_xml` already escapes fields.

---

### HIGH-2: Path traversal in asset cache filenames

**File:** `crates/opensim-services/src/cache.rs`, lines 73, 139, 160, 186
**Severity:** HIGH

Cache file paths are constructed by appending the asset ID directly:

```rust
let path = dir.join(format!("{}.bin", id));
```

The `id` originates from remote grid responses (e.g. `parse_asset_xml`
returning `parsed.id`). A malicious grid could return an asset with
`id = "../../etc/cron.d/evil"`, causing the cache to write to
`/etc/cron.d/evil.bin` (or read from arbitrary paths).

This affects `get()`, `put()`, `contains()`, and `remove()`.

**Recommended fix:** Validate that the ID is a well-formed UUID
(e.g. `Uuid::parse_str(id).is_ok()`) before any cache file operation, or
strip all path separators. A helper like
`id.replace(['/', '\\', '.'], "")` would also work as a belt-and-suspenders
measure.

---

### HIGH-3: No HTTP request timeouts on any remote connector

**Files:**
- `crates/opensim-services/src/remote/asset.rs`, line 31 (`reqwest::Client::new()`)
- `crates/opensim-services/src/remote/inventory.rs`, line 25 (`reqwest::Client::new()`)
- `crates/opensim-services/src/remote/grid.rs`, line 25 (`reqwest::Client::new()`)
- `crates/opensim-services/src/registration.rs`, line 103

**Severity:** HIGH

All remote connectors create `reqwest::Client::new()` with no timeout
configuration. By default, reqwest applies **no** connect or read timeout.
A malicious or unresponsive remote grid can hold connections open
indefinitely (slowloris), exhausting the region server's connection pool
and async runtime threads.

**Recommended fix:** Build the client with explicit timeouts:

```rust
reqwest::Client::builder()
    .connect_timeout(Duration::from_secs(10))
    .timeout(Duration::from_secs(30))
    .build()
    .expect("HTTP client")
```

Consider also setting `pool_max_idle_per_host` and
`pool_idle_timeout` to bound connection reuse.

---

### HIGH-4: No response body size limit

**Files:**
- `crates/opensim-services/src/remote/asset.rs`, lines 46, 81, 110
- `crates/opensim-services/src/remote/inventory.rs`, lines 41, 59, 78
- `crates/opensim-services/src/remote/grid.rs`, lines 57, 81, 110, 133

**Severity:** HIGH

All connectors call `.text().await?` to read the entire response body into
a `String` with no size limit. A malicious remote grid could return a
multi-gigabyte response body, causing out-of-memory.

This is especially concerning for `get_asset`, which reads asset data
(potentially large) plus base64-encodes it in XML.

**Recommended fix:** Use `resp.bytes()` with a length check, or configure a
maximum body size on the reqwest client. For non-asset responses, a 1 MB
limit is generous. For asset data, use the grid's configured maximum asset
size.

---

### MEDIUM-1: `regionSecret` and `Token` transmitted without TLS enforcement

**File:** `crates/opensim-services/src/remote/grid.rs`, lines 32-53
**Severity:** MEDIUM

`register_region` sends `regionSecret` and `Token` as plain-text
URL-encoded form parameters. The connector accepts any scheme in
`server_uri`, including plain HTTP. On an unencrypted connection these
secrets are exposed to network observers.

**Recommended fix:** Warn or refuse if the grid URI scheme is not HTTPS.
At minimum, log a prominent warning at startup when connecting to an
HTTP endpoint.

---

### MEDIUM-2: Sensitive data in error messages

**File:** `crates/opensim-services/src/remote/asset.rs`, lines 85-88
**Severity:** MEDIUM

The `store_asset` error path includes the full server response body:

```rust
Err(RemoteError::Server(format!(
    "unexpected store response: {body}"
)))
```

The response body from a remote grid could contain session tokens,
internal error details, or other sensitive information. This string may
be logged, displayed to operators, or propagated through error chains.

Similarly, `RemoteError::Http` wraps `reqwest::Error`, which can include
request URLs containing sensitive query parameters.

**Recommended fix:** Truncate the body in error messages
(e.g. `&body[..body.len().min(256)]`) and sanitise or redact URLs in error
paths.

---

### MEDIUM-3: No UUID validation on outgoing requests

**Files:**
- `crates/opensim-services/src/remote/asset.rs`, lines 38-39 (get_asset: id in URL path)
- `crates/opensim-services/src/remote/asset.rs`, line 82 (store_asset: verifying response)
- `crates/opensim-services/src/remote/grid.rs`, lines 33, 75 (register/deregister)
- `crates/opensim-services/src/remote/inventory.rs`, lines 35, 54, 73

**Severity:** MEDIUM

None of the connectors validate that string parameters (asset IDs, region
UUIDs, folder IDs, principal IDs) are well-formed UUIDs before using them.
While `reqwest` URL-encodes form parameters, the asset connector
interpolates the ID directly into the URL path:

```rust
let url = format!("{}/assets/{id}", self.base_uri);
```

A malformed `id` like `../../admin/delete` would produce a path traversal
in the URL. reqwest does percent-encode path components by default when
using `Url::parse`, but `format!` + raw string concatenation bypasses that.

**Recommended fix:** Add a `validate_uuid(s: &str) -> Result<&str, ...>`
helper and call it at the entry of every public method on the connectors.
Reject any ID that does not match the UUID format
`[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}`.

---

### MEDIUM-4: Cache poisoning from malicious grid responses

**File:** `crates/opensim-services/src/cache.rs`
**Severity:** MEDIUM

The asset cache stores whatever the remote grid returns, keyed by the
asset ID the caller requested. If a malicious grid returns a different
asset (e.g. wrong type, wrong creator, different data) for a known UUID,
it is cached and served to subsequent callers without integrity checks.

This is an inherent limitation of caching remote data, but the impact is
significant: a poisoned texture or script asset could affect all viewers
connected through this region.

**Recommended fix:**
1. Verify that the returned asset's `id` matches the requested ID.
2. Consider an optional content-hash verification if the protocol
   supports it.
3. For security-sensitive asset types (scripts, objects), bypass the cache
   or add a short TTL.

---

### MEDIUM-5: `register_region` does not check HTTP status code

**File:** `crates/opensim-services/src/remote/grid.rs`, lines 54-61
**Severity:** MEDIUM

Unlike `store_asset` (which calls `.error_for_status()?`),
`register_region` reads the response body without checking the HTTP
status:

```rust
let body = self
    .client
    .post(&self.grid_uri)
    .form(&params)
    .send()
    .await?
    .text()
    .await?;
```

A 500 or 302 response would be silently processed as if it were a success
response, and the XML parsing would attempt to extract `<Result>`. A
redirect (3xx) could silently re-send credentials to a different host if
reqwest follows redirects (it does by default).

The same pattern applies to `deregister_region` (line 76-83) and
`get_neighbours` (lines 105-112).

**Recommended fix:** Add `.error_for_status()?` after `.send().await?`, or
at minimum disable automatic redirect following for these requests:

```rust
reqwest::Client::builder()
    .redirect(reqwest::redirect::Policy::none())
    .build()
```

---

### MEDIUM-6: Redirect following may leak credentials to third parties

**Files:** All remote connectors using `reqwest::Client::new()`
**Severity:** MEDIUM

The default reqwest client follows up to 10 redirects. If the remote grid
returns a 3xx redirect, the client will resend the request (including
form body with `regionSecret`, `Token`, and other sensitive parameters) to
the redirect target. A malicious grid could redirect registration requests
to an attacker-controlled server to capture credentials.

**Recommended fix:** Disable redirect following on all remote-connector
HTTP clients, or at minimum strip sensitive headers/body on cross-origin
redirects.

---

### LOW-1: Y2038 timestamp overflow in `now_secs()`

**File:** `crates/opensim-services/src/remote/asset.rs`, lines 19-24
**Severity:** LOW

```rust
fn now_secs() -> i32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i32)
        .unwrap_or(0)
}
```

The `as i32` cast will overflow on 2038-01-19, producing negative
timestamps. This matches the C# schema's `int` column type, so it is a
protocol-level constraint, but should be documented and monitored.

**Recommended fix:** Use `i64` internally and clamp to `i32::MAX` when
writing to the schema, with a log warning.

---

### LOW-2: `get_neighbours` / `deregister_region` skip `error_for_status()`

**File:** `crates/opensim-services/src/remote/grid.rs`, lines 76-92, 95-115
**Severity:** LOW

Same pattern as MEDIUM-5 but for read-type operations. The
`deregister_region` path is lower risk since no secrets are sent (just
a region ID), but unexpected redirect behaviour or error responses would be
silently misinterpreted.

**Recommended fix:** Same as MEDIUM-5.

---

### LOW-3: Inventory connector only implements read paths (incomplete write coverage)

**File:** `crates/opensim-services/src/remote/inventory.rs`
**Severity:** LOW (informational for future development)

The `RemoteInventoryConnector` only has `get_root_folder`,
`get_folder_content`, and `get_folder_items`. There are no methods for
creating, updating, or deleting inventory items/folders on a remote grid.
The hypergrid trait (`HypergridInventoryService`) defines `add_item`, but
the remote connector does not implement it.

When inventory write methods are added, they will need the same validation
patterns identified in this review (UUID validation, timeout, size limits,
TLS enforcement).

**Recommended fix:** Track as a TODO. When implementing remote inventory
writes, apply all findings from this review as pre-conditions.

---

### INFO-1: Hypergrid service traits are well-designed for security

**File:** `crates/opensim-services/src/hypergrid.rs`
**Severity:** INFO (positive)

The trait definitions encode good security patterns in their API design:
- `HypergridAssetService::store_asset` returns `Option<Uuid>`, allowing
  implementations to reject imports by type.
- The documentation explicitly states that foreign grids may never mutate
  local assets (Delete and UpdateContent refused).
- `HypergridInventoryService` exposes only the Suitcase folder, not the
  real inventory root.
- The `GatekeeperService::login_agent` documentation specifies a
  multi-step verification chain (viewer check, VerifyAgent callback, UUID
  impersonation check, access control).
- All trait methods take typed `Uuid` parameters (not raw strings),
  eliminating UUID format issues at the type level.

The stub implementations use `todo!()`, which panics, safely preventing
accidental use of unimplemented paths.

---

### INFO-2: Positive security patterns observed

The following good practices are already in place:

1. **XML escaping on output:** `write_asset_xml` uses
   `quick_xml::escape::escape()` for all string fields before
   interpolating into XML.
2. **Safe XML parsing:** All XML parsing uses `quick_xml`, which is
   memory-safe and does not process external entities (no XXE risk).
3. **Form encoding:** reqwest automatically URL-encodes form parameters,
   preventing injection in POST bodies.
4. **Cache concurrency:** `AssetCache` uses `tokio::sync::Mutex` to
   protect internal state, preventing data races.
5. **Cache deserialization bounds checking:** `deserialize_asset` validates
   buffer lengths at each read step.
6. **Trailing-slash normalisation:** All connectors strip trailing slashes
   from base URIs in constructors.
7. **HTTP error checking on asset store:** `store_asset` calls
   `error_for_status()` before processing the response.
8. **LRU eviction:** The cache has a configurable maximum size with
   proper eviction, limiting memory consumption from normal use.
9. **Wizard pre-flight checks:** The wizard validates region name, coordinates,
   estate owner, and port availability before attempting registration,
   catching common configuration errors early.

---

## Summary

| Severity | Count | IDs |
|----------|-------|-----|
| CRITICAL | 1     | CRITICAL-1 |
| HIGH     | 4     | HIGH-1, HIGH-2, HIGH-3, HIGH-4 |
| MEDIUM   | 6     | MEDIUM-1 through MEDIUM-6 |
| LOW      | 3     | LOW-1, LOW-2, LOW-3 |
| INFO     | 2     | INFO-1, INFO-2 |

---

## Verdict: CONDITIONAL PASS

The code is structurally sound and demonstrates good Rust safety practices.
The XML parsing, form encoding, and cache concurrency are well-implemented.
The hypergrid trait design is exemplary.

However, **five findings must be addressed before connecting to an untrusted
remote grid:**

1. **CRITICAL-1** (SSRF via neighbour URIs) -- must fix before any external
   grid deployment. An attacker-controlled grid can make the region server
   issue requests to arbitrary internal endpoints.

2. **HIGH-1** (XML injection in `asset_exists`) -- straightforward fix,
   add XML escaping.

3. **HIGH-2** (cache path traversal) -- straightforward fix, validate
   asset IDs as UUIDs.

4. **HIGH-3** (no request timeouts) -- required to prevent denial of
   service from a slow/malicious grid.

5. **HIGH-4** (no response size limits) -- required to prevent
   out-of-memory from oversized responses.

Additionally, **MEDIUM-5** and **MEDIUM-6** (missing `error_for_status()`
and redirect following) should be fixed before production use, as they
create credential-leakage vectors.

All five blocking issues have clear, localised fixes that do not require
architectural changes.
