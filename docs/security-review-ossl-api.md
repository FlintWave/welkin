# Security Review: OSSL API

**Scope:** Tasks 14.1 (Threat-level enforcement) and 14.2 (Crypto review)
**Date:** 2026-11-13
**Reviewer:** Claude Opus 4.6 (automated security review)
**Risk Level:** HIGH
**Files reviewed:** All 13 files under `crates/welkin-script-vm/src/ossl/`

## Executive summary

**CONDITIONAL PASS**: The OSSL API implementation has a sound security
architecture with one high-severity interoperability defect in the crypto
module and one medium-severity gap in the role-based permission system.  No
critical vulnerabilities were found.  The registration-time gating model is
actually stronger than the C# reference (functions that fail the threat-level
check are never exposed to the Lua VM at all, eliminating an entire class of
bypass risks).  The crypto defect prevents cross-implementation decryption and
must be fixed before production use.

| Severity | Count |
|----------|-------|
| CRITICAL | 0     |
| HIGH     | 1     |
| MEDIUM   | 2     |
| LOW      | 2     |
| INFO     | 3     |

---

## Findings

### [S1]: PBKDF2 uses HMAC-SHA256 instead of HMAC-SHA1 (HIGH)

**File:** `crypto.rs:66-82`

**Description:** The `derive_key_iv()` function uses `Hmac<Sha256>` as the PRF
for PBKDF2 key derivation:

```rust
pbkdf2::pbkdf2::<Hmac<Sha256>>(passphrase.as_bytes(), salt, PBKDF2_ITERATIONS, &mut buf)
```

The C# `Rfc2898DeriveBytes` class in .NET Framework (which OpenSimulator
targets) defaults to **HMAC-SHA1**, not HMAC-SHA256.  The code comment on
line 55 ("matches C# Rfc2898DeriveBytes") is incorrect.  The iteration count
(1000) and salt (`b"OpenSimulator OSSL"`) do match the C# defaults, but the
hash function mismatch means:

- Data encrypted by C# OpenSimulator **cannot** be decrypted by this Rust
  implementation (and vice versa).
- `osAESEncrypt`/`osAESDecrypt` will silently produce different ciphertexts
  than the C# version for the same passphrase+plaintext.

**Recommendation:** Change to `Hmac<Sha1>` using the `sha1` crate, or
`Hmac<hmac::digest::core_api::CoreWrapper<sha1::Sha1Core>>`.  Add a
cross-implementation test vector generated from the C# code.

---

### [S2]: Role-based per-function permissions are non-functional at registration time (MEDIUM)

**File:** `mod.rs:369-396` (registration) and `mod.rs:190-192` (`is_allowed_simple`)

**Description:** The `register_ossl()` function gates registration using
`config.is_allowed_simple(func.name, func.level)`, which passes `None` for
the caller role.  When a function has per-function role-based overrides
(i.e., `allowed_roles` is non-empty in `function_perms`), the `is_allowed()`
logic at line 178 denies the function because no caller role is provided:

```rust
// No caller role provided but roles are required -- denied.
return false;
```

This means any function configured with role restrictions via INI
(e.g., `Allow_osNpcCreate = ESTATE_MANAGER,ESTATE_OWNER`) will **never** be
registered into the Lua VM, even for callers who hold the correct role.

The C# reference checks permissions at **runtime** (when the function is
actually called by a script), not at registration time.  The Rust
architecture registers functions once at VM creation, so role-based checks
need a different approach: either register the function unconditionally when
it has a role override and check roles at call time, or register a wrapper
that performs the role check.

**Impact:** More restrictive than intended (fails closed).  No unauthorised
access is possible -- qualified users are incorrectly denied.

**Recommendation:** When `function_perms` contains a role-based override for
a function, register it regardless of threat level.  Inject a role-check
wrapper into the Lua closure that calls `config.is_allowed()` with the
actual caller's `OsslRole` at invocation time.

---

### [S3]: `expect()` panics in crypto primitives (MEDIUM)

**File:** `crypto.rs:78` and `crypto.rs:119`

**Description:** Two `.expect()` calls can panic the server:

1. Line 78: `pbkdf2::pbkdf2::<Hmac<Sha256>>(...).expect("PBKDF2 should not fail")`
2. Line 119: `cipher.encrypt_padded_mut::<NoPadding>(...).expect("encrypt: padded data should be block-aligned")`

While these should not trigger under normal operation (PBKDF2 does not fail
with valid params; the data is explicitly padded before encrypt), a panic in a
production server thread is unacceptable.  If a future refactor introduces a
path where padding is incorrect, the server crashes instead of returning an
error to the script.

**Recommendation:** Convert both to `map_err` returning `mlua::Error::RuntimeError`,
consistent with the error handling in the decrypt path.

---

### [S4]: OsslRole uses flat membership check instead of hierarchy (LOW)

**File:** `mod.rs:101-108` and `mod.rs:172-179`

**Description:** The C# implementation has an implicit role hierarchy where
estate owners automatically pass estate-manager checks, estate managers pass
parcel-owner checks, and so on.  The Rust implementation uses a flat
`Vec<OsslRole>` with `contains()`:

```rust
return perm.allowed_roles.contains(&role);
```

The INI macro system (`expand_macro` at line 200) compensates by expanding
macros like `${OSSL|osslParcelP}` to the full list
`PARCEL_OWNER,PARCEL_GROUP_MEMBER,ESTATE_MANAGER,ESTATE_OWNER`.  However, if
an administrator writes `Allow_osNpcCreate = PARCEL_OWNER` directly (without a
macro), estate owners will be incorrectly denied.

**Impact:** Low.  Administrators using the documented macros are not affected.
Direct role strings may produce surprising results.

**Recommendation:** Implement `OsslRole::implies()` so that higher roles
automatically satisfy lower-role checks, or document the flat-list behaviour
prominently in the configuration guide.

---

### [S5]: Distinct AES error messages could be a padding oracle in non-stub context (LOW)

**File:** `crypto.rs:167-186` and `crypto.rs:218-252`

**Description:** The `osAESDecrypt` and `osAESDecryptFrom` functions return
distinct error messages for different failure modes:

- `"bad base64"` -- base64 decode failure
- `"decryption failed"` -- cipher error
- `"invalid PKCS7 padding"` -- padding validation failure
- `"invalid UTF-8"` -- plaintext encoding failure

In a classic padding oracle attack, distinguishing between "decryption failed"
and "invalid padding" allows byte-by-byte plaintext recovery.

**Mitigating factors:** The attacker provides the passphrase as a function
argument, so they already control the key.  In the current stub
implementation, there is no server-side secret to extract.

**Impact:** None in the current stub context.  Becomes relevant if these
functions are ever connected to server-managed keys.

**Recommendation:** When the stubs become real implementations, unify the
decryption and padding error messages into a single
`"osAESDecrypt: decryption failed"` message.

---

### [S6]: `OsslRole::Anyone` is not parseable from INI (INFO)

**File:** `mod.rs:112-120`

**Description:** `OsslRole::from_name()` does not recognise `"ANYONE"` as a
valid input -- it returns `None` for any string not in the set
`{ESTATE_MANAGER, ESTATE_OWNER, PARCEL_OWNER, PARCEL_GROUP_MEMBER}`.  The
`Anyone` variant exists in the enum but can only be used programmatically.

In the C# reference, an empty role list or `"true"` means "allow anyone".
The Rust INI parser handles `"true"` correctly (adds to `allowed_functions`
instead of `function_perms`), so the same semantics are achieved through a
different path.

**Impact:** None.  The `true` path provides equivalent functionality.

---

### [S7]: Macro expansion loop is theoretically unbounded (INFO)

**File:** `mod.rs:213-229`

**Description:** The `expand_macros()` function loops until no more
`${OSSL|...}` patterns are found.  Currently, no macro expansion contains
another `${OSSL|...}` reference, so termination is guaranteed.  However, if a
future macro is added whose expansion contains a macro reference, this loop
would run indefinitely.

**Impact:** None currently.  Future maintenance risk only.

**Recommendation:** Add a loop counter (e.g., max 10 iterations) with a
warning if exceeded.

---

### [S8]: Default PBKDF2 salt is hardcoded (INFO)

**File:** `crypto.rs:58`

**Description:** `DEFAULT_SALT = b"OpenSimulator OSSL"` is a constant salt
shared by all passphrase-derived keys.  This is a known limitation of the
C# OSSL design and is correctly replicated here.  With a fixed salt,
identical passphrases produce identical keys, enabling rainbow-table attacks
on weak passphrases.

The explicit-IV variants (`osAESEncryptTo`/`osAESDecryptFrom`) allow callers to
provide their own IV, which partially mitigates this for callers who use them.

**Impact:** Inherited from C# design.  Not a defect in this implementation.

---

## Positive observations

- **Registration-time gating is a strong architectural choice.**  Functions
  that fail the threat-level check are never registered into the Lua VM.
  This eliminates an entire class of bypass risks (scripts cannot reference
  or call unregistered functions).  This is actually stronger than the C#
  runtime-check model.

- **No duplicate function names.**  The test `no_duplicate_function_names`
  (mod.rs:1149) explicitly checks for duplicates in the catalog, preventing
  accidental name collisions between modules.

- **Regex DoS is not a concern.**  The `regex` crate uses a guaranteed
  linear-time NFA engine, so user-supplied patterns in `osMatchString`,
  `osReplaceString`, and `osRegexIsMatch` cannot cause exponential
  backtracking.

- **INI parser is robust.**  All malformed input is logged and skipped with
  no panics.  Unknown keys, invalid threat levels, and bad role names all
  produce tracing warnings and preserve the default configuration.

- **All stubs log the calling `script_id`.**  This provides an audit trail
  for which script invoked each function, which is essential for
  investigating abuse.

- **Crypto error messages do not leak key material.**  Error paths expose
  only structural information (base64 validity, padding validity) and never
  include key bytes, IV bytes, or derived key material.

- **PKCS7 padding implementation is correct.**  The pad function always adds
  1..16 bytes (never zero), and the unpad function validates all padding bytes
  match, rejecting truncated or corrupted data.

- **The `function_catalog()` function and `ossl_functions()` share a single
  source of truth.**  There is no risk of a function being registerable but
  absent from the catalog, or vice versa.

- **Threat-level ordering uses `#[derive(PartialOrd, Ord)]` with `#[repr(u8)]`.**
  The compiler enforces that the ordering matches the numeric values, and the
  test `threat_level_ordering` explicitly verifies the full chain.

- **The OsslConfig default is conservatively restrictive.**  `VeryLow` as the
  default max threat level means only the safest functions are available unless
  the administrator explicitly raises the level.

- **Base64 encoding uses `STANDARD` (RFC 4648).**  This is compatible with
  C# `Convert.ToBase64String()` / `Convert.FromBase64String()`.

---

## Appendix: File inventory

| File | Functions | Role |
|------|-----------|------|
| `mod.rs` | 0 direct; registry + config | Core types, INI parser, registration |
| `draw.rs` | 21 | Draw-list string builders (None) |
| `string.rs` | 10 | String/regex operations (VeryLow) |
| `math.rs` | 13 | Math/utility (None) |
| `crypto.rs` | 5 | SHA-256, AES encrypt/decrypt (VeryLow/Low) |
| `time.rs` | 6 | Time functions (None) |
| `sim_info.rs` | 16 | Engine identity, grid info (None/Moderate) |
| `stubs.rs` | 23 | Original stubs (mixed levels) |
| `npc_stubs.rs` | 19 | NPC functions (High) |
| `agent_stubs.rs` | 20 | Agent/teleport (mixed) |
| `terrain_stubs.rs` | 19 | Terrain/environment/parcel (mixed) |
| `inventory_stubs.rs` | 48 | Inventory/attachment/sound/link (mixed) |
| `misc_stubs.rs` | 26 | Remaining stubs (mixed) |
| **Total** | **226** | |
