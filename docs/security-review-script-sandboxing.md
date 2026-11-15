# Security review: script engine sandboxing

**Reviewer:** Claude Opus (second review, HIGH RISK)
**Date:** 2026-11-13
**Verdict:** CONDITIONAL PASS
**Scope:** memory, CPU, HTTP, inventory access, cross-script isolation

## Summary

The Welkin script VM (`welkin-script-vm` crate) runs untrusted LSL/Luau
scripts inside per-instance Luau VMs via mlua 0.12. The architecture is
fundamentally sound: each script gets its own VM (strong isolation), the Luau
language itself lacks dangerous standard libraries (no `io`, `os`, `debug`,
`loadfile`, `dofile`), memory limits use the Luau allocator hook, and CPU
preemption uses the mlua interrupt callback. There are no network-capable or
filesystem-capable functions registered.

However, there is one **HIGH-severity** finding that must be fixed before
production: `ScriptInstance` never calls `lua.sandbox(true)` after registering
handler functions, so the Luau global-freezing and standard-library hardening
is never applied to production VMs. There is also a **HIGH-severity** unbounded
event queue that could be used for host-side memory exhaustion, and several
medium/low findings related to serialization safety, transfer bundle hardening,
and future-proofing of stubs that accept URLs.

## Findings

### CRITICAL (blocks ship)

None.

### HIGH (must fix before production)

**H1. ScriptInstance VMs are never sandboxed**

- **Location:** `instance.rs` lines 123-148 (`ScriptInstance::new`) and lines
  276-299 (`ScriptInstance::restore_from`)
- **Description:** `new_vm()` and `new_vm_with_memory_limit()` in `lib.rs`
  correctly call `lua.sandbox(true)`, but these functions are only used in
  unit tests. The actual `ScriptInstance` constructor creates a `Lua::new()`,
  sets the memory limit, executes bytecode to register handlers, and then
  **never calls `sandbox(true)`**. `restore_from` has the same gap. The doc
  comment on `ScriptInstance` (line 95) incorrectly claims it "Holds a
  sandboxed Luau VM."
- **Attack scenario:** Without `sandbox(true)`, an untrusted script can:
  1. Overwrite registered `ll*` / `os*` host functions with attacker-controlled
     versions (e.g., replace `llSay` with a function that also captures and
     exfiltrates arguments to a global table for later retrieval).
  2. Mutate the Luau standard library tables (`math`, `string`, `table`,
     `coroutine`) - not exploitable for cross-script attacks (separate VMs)
     but violates the principle of least privilege.
  3. In future, if per-environment isolation replaces per-VM isolation (for
     efficiency), the unfrozen globals become a cross-script attack surface.
- **Fix:** After `vm.load(&bytecode).exec()` in both `new()` and
  `restore_from()`, call `vm.sandbox(true)?` to freeze the global table.
  This is safe because handler functions are already registered as globals
  at that point.

**H2. Event queue is unbounded - host-side memory exhaustion**

- **Location:** `instance.rs` line 152, `ScriptInstance::post_event`
- **Description:** `event_queue: VecDeque<ScriptEvent>` has no size cap.
  Each `ScriptEvent::Listen` variant contains heap-allocated `String` fields
  (`name`, `message`). An attacker who controls event sources (e.g., floods
  chat on a listened channel, or rapidly fires collisions via physics) can
  grow the event queue without bound, exhausting host memory before the
  scheduler drains events.
- **Attack scenario:** Send millions of `Listen` events with large message
  bodies to a script that has called `llListen`. The Lua VM's memory limit
  does not apply to the Rust-side event queue.
- **Fix:** Add a configurable `max_event_queue_size` (e.g., 256 or 1024).
  Drop or log-and-discard events beyond the cap, matching the behaviour of
  the C# OpenSimulator script engine which caps at 300 events per script.

### MEDIUM (should fix)

**M1. `restore_script_state` returns an unsandboxed VM to any caller**

- **Location:** `lib.rs` lines 121-134
- **Description:** The function documents that it returns a "non-sandboxed"
  VM, but provides no mechanism or reminder to re-sandbox after globals are
  set. Any new call site that uses `restore_script_state` must remember to
  call `sandbox(true)` after loading bytecode. This is a footgun.
- **Fix:** Either (a) have `restore_script_state` accept a flag to
  auto-sandbox after setting globals, or (b) return a wrapper type that
  forces the caller to explicitly opt into sandboxing before executing code.

**M2. Transfer bundle bytecode loaded without version validation**

- **Location:** `transfer.rs` line 82 via `ScriptInstance::restore_from`,
  and `lib.rs` line 288 (`vm.load(&state.bytecode).exec()`)
- **Description:** `RegionTransferBundle` is deserialized from JSON bytes
  that may arrive from another region. The `bytecode` field is a raw
  `Vec<u8>` loaded directly into the VM via `vm.load()`. While Luau's
  bytecode verifier is designed to reject malformed input, there is no
  secondary validation (e.g., a version tag, a checksum, or a bytecode
  format assertion). A tampered bundle could supply bytecode from a
  different Luau version or a non-Luau binary payload.
- **Attack scenario:** An attacker intercepts or forges a transfer bundle
  and replaces bytecode with a payload crafted to exploit a Luau bytecode
  verifier bug. While no such bug is currently known in mlua/Luau 0.12,
  defence-in-depth argues for validation.
- **Fix:** Add a magic/version header to `ScriptState.bytecode` (e.g., the
  Luau bytecode version byte that Luau already embeds - verify it matches
  the version compiled by this build). Consider HMAC-signing transfer
  bundles.

**M3. Transfer bundle deserialization has no size limit**

- **Location:** `transfer.rs` line 111,
  `RegionTransferBundle::deserialize_from_bytes`
- **Description:** `serde_json::from_slice(bytes)` will attempt to
  deserialize arbitrarily large input. A multi-gigabyte JSON payload could
  exhaust memory before parsing completes.
- **Fix:** Check `bytes.len()` against a configurable maximum (e.g., 64 MiB)
  before deserializing.

**M4. No cap on `globals_json` size during deserialization**

- **Location:** `lib.rs` line 127-128
- **Description:** `serde_json::from_str(&state.globals_json)` parses
  arbitrary JSON into a map of Lua globals. A crafted `globals_json` with
  deeply nested or extremely large tables could cause the Lua VM to
  allocate beyond the memory limit before the limit is set (in
  `restore_from`, the memory limit is set *after* `restore_script_state`
  returns).
- **Attack scenario:** An attacker crafts a transfer bundle where
  `globals_json` contains a 500 MB nested table. The VM is created without
  a memory limit, the globals are injected (consuming 500 MB), and only
  then is the memory limit applied.
- **Fix:** In `ScriptInstance::restore_from()`, set the memory limit on the
  VM *before* injecting globals (requires restructuring
  `restore_script_state` to accept a pre-configured VM), or validate the
  globals JSON size before parsing.

### LOW (nice to have)

**L1. `ScriptEvent::Custom` handler name is unconstrained**

- **Location:** `instance.rs` lines 41-44, 246-254
- **Description:** `ScriptEvent::Custom { name, args }` dispatches to any
  global function named `name`. While the event queue is populated by the
  host (not by scripts), a programming error or a compromised event source
  could inject a Custom event with `name = "error"`, `"print"`, or any
  other global, calling it with attacker-controlled args.
- **Fix:** Validate that custom event names match an allowed pattern (e.g.,
  must start with a letter, must not collide with Lua builtins or ll*/os*
  function names).

**L2. Memory limit of 0 means unlimited**

- **Location:** `instance.rs` lines 131-133
- **Description:** `ScriptInstance::new` treats `memory_limit: 0` as "no
  limit". In production, every script should have a memory limit. There is
  no enforcement or warning when a limit of 0 is used.
- **Fix:** Log a warning when `memory_limit == 0` in non-test builds, or
  refuse to create a ScriptInstance without a limit.

**L3. Tracked globals list grows without deduplication check efficiency**

- **Location:** `instance.rs` lines 262-265
- **Description:** `track_global` uses a linear scan
  (`self.tracked_globals.iter().any(...)`) for deduplication. For scripts
  with many tracked globals this is O(n) per call. Not a security issue,
  but a performance footgun if misused.
- **Fix:** Use a `HashSet` instead of `Vec` for `tracked_globals`.

### INFO (observations)

**I1. Communication functions use in-process channels, not network sockets**

Communication builtins (`llSay`, `llShout`, `llWhisper`, `llRegionSay`,
`llRegionSayTo`, `llOwnerSay`, `llInstantMessage`, `llListen`,
`llListenRemove`, `llListenControl`) now have real implementations that
dispatch structured `ChatMessage` values through `ScriptContext::chat_sender`
(a `tokio::sync::mpsc` channel) to the region chat subsystem. No HTTP client,
TCP socket, or UDP send is present. The `Cargo.toml` contains no networking
dependencies. **No network escape is possible from scripts.** Messages stay
within the simulator process; delivery to viewers is handled by the region
module layer.

**I2. No file-system or raw I/O access from scripts**

About 90 of 236 registered OSSL functions now have real implementations
(NPC, terrain, teleport, sim info, draw, crypto, string, math, time).
The remaining ~17 are stubs that return neutral defaults. Notecard
functions (`osGetNotecard`, `osGetNotecardLine`, `osMakeNotecard`) still
return empty values. No `std::fs` or file-system access exists in any
registered function. **No filesystem escape is possible from scripts.**

**I3. Luau inherently lacks dangerous standard libraries**

mlua with the `luau` feature uses the Luau VM which does not include
`io`, `os`, `debug.getinfo`, `debug.sethook`, `loadfile`, `dofile`, or
`loadstring`. The `require` function exists but without a custom
searcher it cannot load anything. This provides a strong baseline
regardless of whether `sandbox(true)` is called.

**I4. Cross-script isolation is strong (separate VMs)**

Each `ScriptInstance` owns a private `Lua` VM. No shared-VM multi-
tenant arrangement is used. Scripts cannot access each other's globals,
tables, closures, or state. This is the strongest isolation model
available.

**I5. `Arc<RwLock<ScriptContext>>` is safe from deadlocks**

All lock acquisitions in registered ll*/os* functions are brief, non-
nested, and bounded. A script cannot hold the lock - it is acquired and
released within the Rust callback, outside the script's control. No
deadlock scenario is possible with the current registration pattern.

**I6. CPU preemption is effective**

The scheduler's interrupt-callback pattern (`set_interrupt` /
`remove_interrupt` around each `run_next_event`) correctly limits
per-event instruction count. The interrupt fires on Luau backward
branches (loops), which is standard practice. The `fetch_add` counter
with `Ordering::Relaxed` is correct for single-threaded-per-VM usage.
Test `instruction_limit_produces_error` confirms the mechanism works.

**I7. `osSetDynamicTextureURL` accepts a URL parameter - future SSRF risk**

Currently a stub, but when implemented it will fetch a URL from within
the simulator. The implementation must validate the URL scheme (reject
`file://`, `gopher://`, etc.) and block internal/loopback IPs to prevent
SSRF. Similarly, `osSetParcelMediaURL` and `osDrawImage` accept URL
strings that may be rendered by viewers.

**I8. OSSL threat-level gating works correctly**

The `OsslConfig` and `register_ossl` mechanism correctly gates
function registration by threat level. Functions above the configured
maximum are never registered (never become Lua globals). The
`allowed_functions` override mechanism works as designed.

## Recommendations

1. **[HIGH, H1]** Add `vm.sandbox(true)?` after bytecode execution in both
   `ScriptInstance::new()` and `ScriptInstance::restore_from()`. Add a test
   that verifies a script cannot overwrite ll* globals after construction.

2. **[HIGH, H2]** Add a configurable event queue cap to `ScriptInstance`
   (default 300, matching OpenSimulator). Drop events beyond the cap with a
   debug log message.

3. **[MEDIUM, M1]** Refactor `restore_script_state` to either accept a
   sandbox flag or return a builder that enforces sandboxing before
   execution.

4. **[MEDIUM, M2]** Validate Luau bytecode version byte before loading
   from transfer bundles. Consider HMAC-signing bundles for inter-region
   transfers.

5. **[MEDIUM, M3/M4]** Add size checks before deserializing transfer
   bundles and globals JSON. Set the memory limit on the VM before
   injecting globals from untrusted sources.

6. **[LOW, L1]** Validate `ScriptEvent::Custom` handler names against
   an allow-list pattern.

7. **[INFO, I7]** When implementing `osSetDynamicTextureURL`, add URL
   scheme validation and internal-IP blocking to prevent SSRF.
