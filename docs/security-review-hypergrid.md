<!-- SPDX-License-Identifier: LGPL-3.0-or-later -->

# Hypergrid Security Review — CRITICAL RISK

Date: 2026-11-14
Reviewer: Claude Opus (automated)
Scope: Hypergrid services, connectors, and region modules

## Overall Assessment

**CONDITIONAL PASS** — with mandatory remediation of all CRITICAL and HIGH
findings before any public-facing deployment. The Hypergrid implementation
correctly models the major security boundaries (suitcase isolation, asset
type filtering, service-session tokens, viewer/MAC deny lists) but has
several gaps that an attacker can exploit to bypass these controls.

Three findings are CRITICAL: the suitcase `get_item` escape, the missing
`VerifyAgent` callback in the gatekeeper, and the missing UUID-impersonation
check. Two are HIGH: non-constant-time token comparison and missing rate
limiting. Five are MEDIUM or LOW.

---

## Findings

### F-01 [CRITICAL] Suitcase folder escape via `get_item`

- **File**: `crates/welkin-services/src/hg_inventory.rs`
- **Lines**: 337–353 (`HGSuitcaseInventoryServiceImpl::get_item`)
- **Risk**: The strict suitcase service validates suitcase-tree membership
  in `get_folder_content` (line 300) and `add_item` (line 355), but
  `get_item` performs **no suitcase-tree check**. It matches only on
  `item_id` and `owner_id`. A foreign grid that learns or guesses an
  `item_id` outside the suitcase (e.g. from a previously shared object's
  metadata, or from a brute-force scan) can retrieve that item directly,
  defeating the entire suitcase containment model.
- **Impact**: Full read access to any inventory item owned by a traveling
  user, including items in "My Inventory" that the user never placed in
  their suitcase. Sensitive scripts, no-transfer objects, and private
  notecards are all exposed.
- **Recommendation**: Add an `is_within_suitcase_tree` guard to `get_item`:

  ```rust
  fn get_item(&self, principal_id: Uuid, item_id: Uuid) -> HgResult<Option<HgInventoryItem>> {
      let items = self.items.read().unwrap();
      let Some(item) = items.iter().find(|i| {
          i.item_id == item_id && i.owner_id == principal_id
      }) else {
          return Ok(None);
      };
      // --- ADD THIS CHECK ---
      if !self.is_within_suitcase_tree(principal_id, item.folder_id) {
          return Ok(None);
      }
      // ----------------------
      let mut out = item.clone();
      self.adjust_creator(&mut out);
      Ok(Some(out))
  }
  ```

  The basic `HGInventoryServiceImpl` should also be reviewed; it performs no
  tree validation at all on `get_item` or `get_folder_content`, relying on
  security-through-obscurity (the caller must know folder IDs). If the basic
  service is meant for trusted use only, document that constraint; otherwise
  add equivalent guards.

---

### F-02 [CRITICAL] `VerifyAgent` callback not implemented in gatekeeper

- **File**: `crates/welkin-services/src/gatekeeper.rs`
- **Lines**: 336–341 (`login_agent`, NOTE comment block)
- **Risk**: The gatekeeper's `login_agent` method validates the
  service-session token *format* (embedded URL matches this grid) and checks
  viewer/MAC/ID0 deny lists, but it **never calls the home grid's
  `UserAgentService::verify_agent`**. The code explicitly marks this as
  "NOTE: the full implementation would additionally" — meaning the callback
  is a documented TODO, not an implemented check.

  An attacker can fabricate an `AgentCircuitInfo` with:
  - `service_session_id` = `"http://target-grid:9000/;<any-uuid>"`
  - Any `agent_id`, `session_id`, names, etc.

  Because the gatekeeper only checks that the URL prefix matches itself, the
  fabricated token passes `check_address`, and no callback verifies the
  session actually exists on any home grid.

- **Impact**: Any external actor can inject an agent onto the grid without
  authentication. Combined with F-03 (no UUID impersonation check), this
  allows full impersonation of any user, including grid administrators.
- **Recommendation**: Wire `login_agent` to call the home grid's
  `UserAgentService::verify_agent(session_id, service_session_id)` via the
  `UserAgentServiceConnector`. Reject the agent if verification fails or
  the home grid is unreachable.

---

### F-03 [CRITICAL] UUID impersonation check not implemented

- **File**: `crates/welkin-services/src/gatekeeper.rs`
- **Lines**: 336–341 (same NOTE block as F-02)
- **Risk**: The gatekeeper does not check whether an incoming agent's UUID
  collides with a local user account. In the C# implementation, this check
  calls `IsAgentComingHome` on the home grid's UserAgentService; if the
  answer is "no", the gatekeeper rejects the agent to prevent a foreign
  user from impersonating a local resident.

  Without this check, a malicious foreign grid can send an agent whose UUID
  matches a local admin or estate owner, gaining all permissions associated
  with that account.

- **Impact**: Privilege escalation to any local account, including god-mode
  administrators.
- **Recommendation**: After `verify_agent` succeeds (F-02), query the local
  user-account service for the agent's UUID. If a local account exists,
  call `UserAgentService::is_agent_coming_home` on the home grid. Reject
  with `HypergridError::UuidImpersonation` if the answer is false.

---

### F-04 [HIGH] Session token comparison is not constant-time

- **File**: `crates/welkin-services/src/user_agent.rs`
- **Line**: 183 (`verify_agent`)
- **Risk**: Token verification uses Rust's `==` operator on strings:

  ```rust
  let matches = travel.service_token == token;
  ```

  String equality short-circuits on the first differing byte. An attacker
  who can measure response times with sufficient precision can recover the
  service token one byte at a time (timing side-channel).

  The crate already provides a `constant_time_eq` function in
  `crates/welkin-services/src/lib.rs` (lines 3–14), but it is not used
  here.

- **Impact**: Service-token theft via timing oracle; combined with the
  lack of token expiration (F-05), a recovered token enables persistent
  session hijacking.
- **Recommendation**: Replace `==` with the existing `constant_time_eq`:

  ```rust
  let matches = crate::constant_time_eq(&travel.service_token, token);
  ```

---

### F-05 [HIGH] No session token expiration or TTL

- **File**: `crates/welkin-services/src/user_agent.rs`
- **Lines**: 43–46, 59–68 (traveling_data store and retrieval)
- **Risk**: `TravelingAgentInfo` records have no `created_at` or `expires_at`
  field. Once stored, a record persists in the `traveling_data` map until
  the agent explicitly logs out via `logout_agent`. If a session token is
  stolen (or if the user disconnects without a clean logout), the token
  remains valid indefinitely, enabling replay.

- **Impact**: Persistent session hijacking after token theft or unclean
  disconnect.
- **Recommendation**: Add a `created_at: Instant` field to
  `TravelingAgentInfo` and enforce a configurable TTL (e.g. 24 hours) in
  `verify_agent` and `verify_client`. Run a periodic sweep to purge
  expired records.

---

### F-06 [HIGH] No rate limiting on any Hypergrid endpoint

- **Files**:
  - `crates/welkin-services/src/gatekeeper.rs` (lines 472–487, `handle_xmlrpc`)
  - `crates/welkin-services/src/user_agent.rs` (lines 351–527, `handle_xmlrpc`)
  - `crates/welkin-services/src/hg_friends.rs` (lines 375–470, `handle_hg_friends`)
- **Risk**: All Hypergrid HTTP handlers accept unlimited requests with no
  per-IP or per-session throttling. An attacker can:
  - Brute-force session tokens via `verify_agent` (especially given the
    non-constant-time comparison in F-04).
  - Enumerate inventory items via `get_item` (especially given the
    suitcase escape in F-01).
  - DoS the gatekeeper, user-agent, or friends service with high request
    volume.
- **Impact**: Denial of service; amplification of other vulnerabilities by
  enabling high-speed brute-force or enumeration.
- **Recommendation**: Add an axum middleware layer (e.g. `tower::limit` or
  a custom `RateLimit` layer) to all Hypergrid routers. Configure per-IP
  and per-session limits. Consider exponential backoff on authentication
  failures.

---

### F-07 [MEDIUM] `assets_exist` leaks asset existence regardless of export permissions

- **File**: `crates/welkin-services/src/hg_assets.rs`
- **Lines**: 136–139 (`HGAssetServiceImpl::assets_exist`)
- **Risk**: `get_asset` and `get_metadata` correctly check
  `allowed_export(asset.asset_type)`, but `assets_exist` returns `true` for
  any asset that exists in the store, regardless of type. A foreign grid can
  probe for the existence of script assets, even though the actual data
  cannot be retrieved.

- **Impact**: Information disclosure — reveals which asset IDs exist on the
  grid, which may leak information about user activity or grid content.
- **Recommendation**: Filter `assets_exist` through the export-permission
  check: return `false` for assets whose type is not in
  `allowed_export_types`.

---

### F-08 [MEDIUM] Friendship secret has only 32 bits of entropy

- **File**: `crates/welkin-modules/src/hg/friends.rs`
- **Line**: 149 (`store_hg_friendship`)
- **Risk**: The shared secret for cross-grid friendships is generated by
  truncating a UUIDv4 to 8 hex characters:

  ```rust
  let secret = Uuid::new_v4().to_string()[..8].to_string();
  ```

  This yields only 32 bits of entropy (4 bytes = 8 hex chars). An attacker
  who knows a friendship exists can brute-force the secret in ~2^31 attempts
  on average — feasible in minutes without rate limiting (F-06).

- **Impact**: Unauthorized friendship deletion; with additional work,
  potential for forging status-notification subscriptions.
- **Recommendation**: Use the full UUID (128 bits) as the secret, or at
  minimum 16 hex characters (64 bits). Combined with rate limiting (F-06),
  this makes brute-force infeasible.

---

### F-09 [MEDIUM] Friendship deletion uses weak `starts_with` + `ends_with` matching

- **File**: `crates/welkin-services/src/hg_friends.rs`
- **Lines**: 197–213 (`HGFriendsServiceImpl::delete_friendship`)
- **Risk**: The deletion logic matches on:

  ```rust
  finfo.friend.starts_with(&friend_str) && finfo.friend.ends_with(secret)
  ```

  If the secret string appears as a suffix of the UUI (e.g. a UUID that
  happens to end with the same characters as the secret), a wrong secret
  could match. While unlikely given random UUIDs, this is a pattern-match
  weakness that should use exact field extraction rather than substring
  matching.

- **Impact**: Low probability of false-positive deletion, but the principle
  of exact matching is violated.
- **Recommendation**: Parse the friend UUI into structured fields and
  compare the secret field exactly, rather than using suffix matching.

---

### F-10 [LOW] MAC and ID0 deny lists use substring matching

- **File**: `crates/welkin-services/src/gatekeeper.rs`
- **Lines**: 265–282 (`login_agent`, MAC and ID0 checks)
- **Risk**: The MAC deny check uses `macs_lower.contains(&mac_lower)` on
  the comma-separated deny string. A short MAC fragment (e.g. `"aa"`) would
  match any MAC containing those characters, causing false-positive denials.
  The same applies to the ID0 check.

  This matches the C# behaviour (which has the same substring-match logic),
  so it is a ported bug rather than a regression.

- **Impact**: False-positive denials for legitimate users whose MAC or ID0
  happens to contain a substring of a banned value.
- **Recommendation**: Split the deny list on commas and compare each entry
  individually against the full MAC/ID0 value, rather than using substring
  containment on the concatenated string.

---

### F-11 [LOW] No per-asset transfer-permission check

- **File**: `crates/welkin-services/src/hg_assets.rs`
- **Lines**: 91–105 (`get_asset`)
- **Risk**: The HG asset service filters by asset *type* (textures, sounds,
  etc.) but does not check per-asset permission flags (e.g. the SL
  "no-transfer" flag). This matches the C# OpenSim design, where per-asset
  permission enforcement is delegated to the inventory layer, but it means
  the asset service itself does not prevent export of a no-transfer asset
  if the caller supplies a valid asset ID.

- **Impact**: Documented design assumption; not a regression from C#. Risk
  depends on the inventory layer correctly enforcing transfer restrictions
  before exposing asset IDs to foreign grids.
- **Recommendation**: Document this as an architectural invariant. Consider
  adding an optional per-asset permission check as defence in depth.

---

## Summary Table

| ID   | Severity | Title                                            | Status       |
|------|----------|--------------------------------------------------|--------------|
| F-01 | CRITICAL | Suitcase folder escape via `get_item`            | Must fix     |
| F-02 | CRITICAL | `VerifyAgent` callback not implemented           | Must fix     |
| F-03 | CRITICAL | UUID impersonation check not implemented         | Must fix     |
| F-04 | HIGH     | Token comparison is not constant-time            | Must fix     |
| F-05 | HIGH     | No session token expiration or TTL               | Must fix     |
| F-06 | HIGH     | No rate limiting on HG endpoints                 | Must fix     |
| F-07 | MEDIUM   | `assets_exist` leaks existence info              | Should fix   |
| F-08 | MEDIUM   | Friendship secret has only 32 bits of entropy    | Should fix   |
| F-09 | MEDIUM   | Friendship deletion uses weak pattern matching   | Should fix   |
| F-10 | LOW      | MAC/ID0 deny lists use substring matching        | Nice to have |
| F-11 | LOW      | No per-asset transfer-permission check           | Document     |

---

## Mandatory Remediation Before Deployment

1. **F-01**: Add suitcase-tree check to `get_item` in
   `HGSuitcaseInventoryServiceImpl`.
2. **F-02 + F-03**: Wire `VerifyAgent` and UUID-impersonation checks into
   `GatekeeperServiceImpl::login_agent`.
3. **F-04**: Switch `verify_agent` to use `constant_time_eq`.
4. **F-05**: Add TTL to traveling-agent records.
5. **F-06**: Add rate-limiting middleware to all HG axum routers.

Items F-07 through F-09 should be addressed in the next development cycle.
Items F-10 and F-11 are low priority but should be tracked.

---

*Review generated by automated security analysis. Manual verification of
each finding against the live codebase is recommended before applying
fixes.*
