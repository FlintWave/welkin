# OSSL API summary

Change: `openspec/changes/ossl-api` (18/18 tasks, accepted as CONDITIONAL PASS).
Code: `crates/opensim-script-vm/src/ossl/` (13 files, about 5,900 lines).
Review: `docs/security-review-ossl-api.md`.

## What was built

- The `os*` extension layer for Luau scripts, grown from 27 functions to 233
  registered functions. `function_catalog()` lists them; the C# reference
  (`OSSL_Api.cs`) has 258 unique names, and the remainder are deprecated aliases
  and overloads folded into one registration.
- An 8-level `OsslThreatLevel` (None to Severe) mirroring the C# levels.
- Per-function overrides (`Allow_<Function>`) with role and UUID allow-lists.
- `[OSSL]` INI loading with `osslParcelO`, `osslParcelOG` and `osslNPC` macros.
- 29 ported C# stub-verification tests (NPC, attachment, appearance) plus a
  catalog test (no duplicates, every threat level represented).

## Architecture

| File | Contents |
|---|---|
| `ossl/mod.rs` | `OsslThreatLevel`, `OsslConfig`, `FunctionPerm`, `OsslRole`, `register_ossl()`, `function_catalog()`, INI parser |
| `draw.rs` | Draw-list functions (21): pure string building, threat None |
| `string.rs` | Regex and string helpers, `osFormatString`, `osMakeHash` |
| `math.rs` | `osMin`/`osMax`, vector and rotation helpers, slerp, list sort |
| `crypto.rs` | `osSHA256`, `osAESEncrypt`/`Decrypt` (AES-256-CBC, PKCS7, PBKDF2 key and IV) |
| `time.rs` | PST wallclock, apparent time |
| `sim_info.rs` | Engine, physics, grid-info, region size and memory |
| `*_stubs.rs`, `stubs.rs` | NPC, agent, inventory, terrain and the rest (see below) |

Config model: `OsslConfig { os_functions_enabled, max_threat_level,
function_perms: HashMap<String, FunctionPerm>, permission_error_to_owner }`.
`FunctionPerm` is `Disabled`, `Everyone` or `AllowList(AllowedRoles)`; roles are
`GridGod`, `God`, `ActiveGod`, `EstateManager`, `EstateOwner`, `ParcelOwner`,
`ParcelGroupMember`. `is_allowed()` combines the global threat ceiling with the
per-function override.

Gating happens at registration: a function that fails the check is never put in
the Lua globals, so a script cannot reach it. The C# engine checks at call time.

## Key decisions

See O1 to O6 in `docs/decisions.md`. In short: registration-time gating;
threat ceiling plus per-function override; macros expanded at INI load; stubs
for functions whose subsystem does not exist yet; AES key derivation must match
.NET `Rfc2898DeriveBytes` (PBKDF2-HMAC-SHA1, 1000 iterations, fixed salt
`OpenSimulator OSSL`).

### PBKDF2 hash fix (SHA256 to SHA1)

The first implementation used `Hmac<Sha256>` in `derive_key_iv()` with a comment
claiming C# parity. .NET Framework `Rfc2898DeriveBytes` uses HMAC-SHA1, so
ciphertext from either implementation could not be decrypted by the other. The
fix swaps the PRF to `Hmac<Sha1>` (`sha1` crate added to
`opensim-script-vm/Cargo.toml`). At the time of this summary the change to
`crypto.rs` was in the working tree and not yet committed; a C#-generated test
vector is still needed to prove interoperability.

## Security review findings

CONDITIONAL PASS. 0 CRITICAL, 1 HIGH, 2 MEDIUM, 2 LOW, 3 INFO.

| ID | Sev | Finding |
|---|---|---|
| S1 | HIGH | PBKDF2 used SHA256, not SHA1; breaks C# interop (fix above) |
| S2 | MEDIUM | Role-based per-function permissions are not enforced, because gating runs at registration when no caller context exists |
| S3 | MEDIUM | `expect()` panics in crypto primitives |
| S4 | LOW | `OsslRole` membership is flat, not hierarchical |
| S5 | LOW | Distinct AES error messages could form a padding oracle once functions are no longer stubs |
| S6 | INFO | `OsslRole::Anyone` not parseable from INI |
| S7 | INFO | Macro expansion loop is not provably bounded |
| S8 | INFO | PBKDF2 salt is hardcoded (matches C#) |

Until S1 is committed and S2 is resolved, treat role-restricted functions as
threat-level gated only.

## Deferred and stubbed

Stubs log with `tracing::debug!` and return a neutral default (empty string,
nil UUID, 0, or nothing). Each is filled in by the change that delivers its
subsystem.

- NPC (24), parcel/land (12), attachments (11), inventory bridge (17),
  teleport (7), dynamic textures (7), sound (11), avatar animation (2),
  environment (4), terrain (5), region restart/notice, kick, groups, console,
  HTTP URLs, particles.
- `osGetDrawStringSize` needs font metrics; `osGetRegionStats` and
  `osGetMapTexture` need the scene and map modules.
- Role enforcement at call time (S2), pending a caller context on `ScriptContext`.
- `osWeatherMap` was removed; it is not in C# OSSL.
