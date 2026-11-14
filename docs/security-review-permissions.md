# Security Review: DefaultPermissionsModule

**Reviewer**: Claude Opus (adversarial)
**Date**: 2026-11-13
**Scope**: `crates/opensim-modules/src/permissions/mod.rs`, `crates/opensim-modules/src/permissions/checks.rs`
**Verdict**: CONDITIONAL PASS

The module is structurally sound and follows the C# `DefaultPermissionsModule` pattern.
However, several issues range from a critical nil-UUID ownership bypass to multiple
medium-severity gaps in group permission logic. The module must not be deployed to a
production multi-user grid without addressing at least the Critical and High findings.

---

## Critical findings (blocking)

### C-1. Nil UUID matches nil object owner — ownership bypass

**Severity**: CRITICAL
**File**: `checks.rs` lines 130, 167, 195, 226, 236, 265, 270, 295, 330, 346, 387, 450, 474
**Affects**: `can_delete_object`, `can_edit_object`, `can_move_object`, `can_copy_object`,
`can_transfer_object`, `can_take_object`, `can_return_object`, `can_create_object`,
`can_run_script`, `can_edit_parcel`, `can_edit_estate`

**Description**: Every permission check that compares `agent_id` against an owner field
(`object_owner`, `parcel.owner_id`, `self.estate_owner`) does so without first guarding
against `agent_id == Uuid::nil()`. While `is_god()`, `is_administrator()`, and
`is_estate_manager()` all reject nil UUIDs, the direct owner-equality comparisons do not.

`LandData::default()` initialises `owner_id` to `Uuid::nil()`. The
`DefaultPermissionsModule` itself initialises `estate_owner` to `Uuid::nil()`. Any object
or parcel whose owner was never explicitly set retains a nil owner UUID.

**Exploit scenario**: An attacker who can present `Uuid::nil()` as their agent ID (e.g.
through a malformed login, a race condition during session setup, or a crafted packet with
a zeroed agent field) passes the owner check against any nil-owned object, parcel, or
estate. With bypass off, they can delete, edit, move, copy, take, and transfer any
nil-owned object, edit nil-owned parcels, and issue estate commands on a region whose
estate owner was never set.

Trace through `can_delete_object(Uuid::nil(), Uuid::nil(), Uuid::nil())`:
1. `bypass_check()` returns `None` (bypass is off).
2. `god_bypass(Uuid::nil())` -> `is_god(Uuid::nil())` -> `is_nil()` -> returns `false` -> `None`.
3. `agent_id == object_owner` -> `Uuid::nil() == Uuid::nil()` -> `true`. **Returns true.**

**Recommended fix**: Add `if agent_id.is_nil() { return false; }` as the first line of
every public permission check function, before bypass_check. Alternatively, add it once
inside `bypass_check()` itself (returning `Some(false)` for nil agents even when bypass
is on), though this changes bypass semantics.

---

## High findings

### H-1. `is_god()` bypasses `allow_grid_gods` gate — god-mode escalation

**Severity**: HIGH
**File**: `mod.rs` lines 183-201

**Description**: `is_grid_administrator()` (line 131) correctly gates the `god_users`
set behind the `allow_grid_gods` config flag. However, `is_god()` (line 189) checks
`self.god_users.contains(&agent_id)` directly, without consulting `allow_grid_gods`.

This means a UUID in the `god_users` set is **always** treated as a god, even when
`allow_grid_gods = false`. The `god_bypass()` function calls `is_god()`, so this UUID
gets god-mode bypass on every permission check.

This creates an inconsistency: the user is god but not administrator. They bypass all
permission checks via `god_bypass()` but `is_administrator()` returns false for them.

**Exploit scenario**: A grid operator sets `allow_grid_gods = false` to restrict admin
powers to estate owners only. However, any UUID remaining in the `god_users` set (perhaps
from a previous configuration, a migration, or programmatic insertion) still receives
full god-mode bypass on all 13 permission checks.

**Recommended fix**: Gate the `god_users` check in `is_god()` behind
`self.config.allow_grid_gods`, matching `is_grid_administrator()`:

```rust
if self.config.allow_grid_gods && self.god_users.contains(&agent_id) {
    return true;
}
```

### H-2. No god-mode request gate — automatic privilege escalation for all admins

**Severity**: HIGH
**File**: `checks.rs` lines 48-57, `mod.rs` lines 183-201

**Description**: In the C# implementation, god-mode bypass in permission checks
requires that the `ScenePresence.IsGod` flag is true. This flag is only set when a
user with `UserLevel >= 200` explicitly sends a `GrantGodlikePowers` request through
the viewer UI. This two-step design acts as a safety switch: admins must consciously
activate god-mode.

The Rust port has no equivalent gate. `is_god()` returns true for any administrator
when `automatic_gods` or `implicit_gods` is set, or for any UUID in `god_users`,
without requiring an explicit activation step. The `god_bypass()` function in
`checks.rs` then grants unrestricted access on every permission check.

**Exploit scenario**: An estate owner (who is an admin via `region_owner_is_god=true`)
inadvertently deletes, moves, or modifies objects belonging to other residents because
god-mode is always active. There is no viewer-side "enter god mode" step to provide a
moment of deliberation. If an attacker compromises an admin account, they immediately
have full god-mode without any additional escalation step.

**Recommended fix**: Add a `god_mode_active: HashSet<Uuid>` field to the module that
tracks which users have explicitly requested god-mode. `is_god()` should require both
administrator status AND presence in this set (or in `god_users`). Provide
`request_god_mode(agent_id)` and `release_god_mode(agent_id)` methods to toggle it.

### H-3. Default configuration has bypass ON — silent open access

**Severity**: HIGH
**File**: `mod.rs` lines 62-78

**Description**: `PermissionsConfig::default()` sets `bypass_permissions: true` and
`bypass_permissions_value: true`. This means a `DefaultPermissionsModule::new()` that
is used without calling `initialise()` (or with a config that lacks the
`serverside_object_permissions` key) allows every action for every agent.

The `initialise()` method reads the config with `get_bool(section,
"serverside_object_permissions", true)`, so the *initialised* default is
`bypass_permissions = false` (since `serverside_object_permissions` defaults to `true`
and bypass is its inverse). However, if a code path constructs the module and skips
initialisation, or if the config source is empty, bypass remains on.

**Exploit scenario**: A developer instantiates `DefaultPermissionsModule::new()` for a
test or a quick setup and forgets to call `initialise()`. All permission checks silently
return `true`. The region runs with no access control.

**Recommended fix**: Change the default for `bypass_permissions` to `false` in
`PermissionsConfig::default()`. Code that explicitly wants bypass mode should set it.
Defence in depth: log a warning at `WARN` level if `bypass_permissions` is true when
the first permission check is called.

---

## Medium findings

### M-1. `is_estate_manager()` not gated by `force_grid_gods_only`

**Severity**: MEDIUM
**File**: `mod.rs` lines 142-150, `checks.rs` lines 335, 404, 435

**Description**: When `force_grid_gods_only = true`, `initialise()` correctly sets
`region_owner_is_god = false` and `region_manager_is_god = false`, which strips the
estate owner and managers from the `is_administrator()` path. However,
`is_estate_manager()` is a separate function that always returns true for the estate
owner and anyone in `estate_managers`, regardless of `force_grid_gods_only`.

Functions that call `is_estate_manager()` directly (not through `is_administrator()`)
still grant elevated privileges to estate owners/managers:

- `generic_estate_permission()` (line 204) -> used by `can_edit_terrain()`,
  `can_run_script()` (estate fallback)
- `can_return_object()` (line 335)

This is consistent with the C# behaviour where `GenericEstatePermission` checks
`IsEstateManagerOrOwner` independently. However, it undermines the intent of
`force_grid_gods_only` if the operator expects it to fully strip estate-level
privileges.

**Recommended fix**: Document this behaviour explicitly. If full restriction is
desired, consider adding a config flag to also gate `is_estate_manager()` checks, or
document that `force_grid_gods_only` only affects administrator/god status, not basic
estate management functions.

### M-2. Group permission checks are no-ops or use incorrect comparisons

**Severity**: MEDIUM
**File**: `checks.rs` lines 101-108, 140-143, 304-307, 392-400

**Description**: Multiple permission checks document group logic as "simplified" but
the implementations are either no-ops or semantically wrong:

| Function | Issue |
|---|---|
| `can_create_object` (line 103) | Compares `agent_id == parcel.group_id`. An agent UUID will never equal a group UUID. Dead code. |
| `can_delete_object` (line 140-143) | Comment says "simplified check" but the block is empty. Group members cannot delete group-owned objects. |
| `can_take_object` (line 304-307) | Same as above — empty block. Group members cannot take group-owned objects. |
| `can_run_script` (line 398) | Compares `agent_id == parcel.group_id`. Same agent-vs-group-UUID mismatch as `can_create_object`. |
| `can_edit_object` | `_object_group` parameter is unused. No group logic at all. |
| `can_move_object` | `_object_group` parameter is unused. No group logic at all. |
| `can_copy_object` | `_object_group` parameter is unused. No group logic at all. |

**Exploit scenario**: In a grid where group-owned land and objects are common (e.g.
club parcels, community builds), group members have no group-based permissions.
They cannot rez on group land with `CREATE_GROUP_OBJECTS`, cannot delete or take
group-owned objects, and scripts owned by group members cannot run on
`ALLOW_GROUP_SCRIPTS` parcels. This causes functionality loss rather than privilege
escalation, but the incorrect `agent_id == group_id` comparisons could mask future
bugs if group logic is partially implemented.

**Recommended fix**: Either implement proper group membership checks (requires a
group service interface) or replace the dead code with explicit `// TODO: requires
group service` comments and ensure the functions return `false` for group paths
rather than having unreachable code that appears to grant access.

### M-3. Non-owner permission checks only examine owner mask

**Severity**: MEDIUM
**File**: `checks.rs` lines 151-176, 181-206, 211-246

**Description**: `can_edit_object`, `can_move_object`, and `can_copy_object` receive
a single `object_perms: PermissionMask` parameter. For owners, they check the
appropriate bit (MODIFY, MOVE, COPY). For non-owners who are not administrators, they
return `false`.

In the C# implementation, objects have separate permission masks: `OwnerMask`,
`GroupMask`, `EveryoneMask`, and `NextOwnerMask`. A non-owner, non-admin user can
edit an object if the `EveryoneMask` has the MODIFY bit set (e.g. a public sandbox
object set to "anyone can modify"). The Rust port has no way to express this because
it only passes one mask.

The `ObjectPerms` struct (lines 20-27) is defined with `owner_id`, `group_id`, and
`owner_mask`, but no `everyone_mask` or `group_mask` fields, and the struct is never
actually used by any check function.

**Exploit scenario**: Objects deliberately set to "everyone can edit" (EveryoneMask
includes MODIFY) cannot be edited by non-owners. This is a denial of legitimate
functionality, not a privilege escalation.

**Recommended fix**: Extend `ObjectPerms` to include `everyone_mask` and
`group_mask`. Refactor the check functions to accept `&ObjectPerms` and consult the
appropriate mask for the relationship between agent and object.

### M-4. `can_compile_script` missing bypass check

**Severity**: MEDIUM
**File**: `checks.rs` lines 410-417

**Description**: `can_compile_script` does not call `bypass_check()` or
`god_bypass()`. All other permission check functions start with these two
short-circuits. When `bypass_permissions = true`, every other check returns the
bypass value, but `can_compile_script` still enforces the
`allowed_script_creators` restriction.

In the C# source, `CanCompileScript` also does not check bypass, and the bypass
is handled by the calling code. However, since the Rust port presents all checks
as methods on `DefaultPermissionsModule` with a consistent API contract, the
inconsistency is surprising and may lead to bugs when callers assume all methods
respect bypass.

**Exploit scenario**: An operator enables `bypass_permissions = true` for testing
and sets `allowed_script_creators = Administrators`. Script compilation fails for
non-admin testers despite bypass being on, causing confusion and potentially
blocking development workflows.

**Recommended fix**: Add `bypass_check()` and `god_bypass()` to
`can_compile_script()` for API consistency, or document the exception prominently
in the function's doc comment.

---

## Low findings

### L-1. `can_edit_estate` does not check estate managers

**Severity**: LOW
**File**: `checks.rs` lines 465-483

**Description**: `can_edit_estate` checks only the estate owner (line 474) and
administrators (line 478). It does not call `is_estate_manager()` or
`generic_estate_permission()`. This means estate managers cannot issue estate
commands unless they are also administrators.

In the C# `CanIssueEstateCommand`, the check includes estate managers. The Rust
port's omission means an estate manager who is not an admin (and
`region_manager_is_god = false`) cannot issue estate commands.

**Recommended fix**: Add `self.is_estate_manager(agent_id)` check, or replace
the manual checks with `self.generic_estate_permission(agent_id)`.

### L-2. `can_run_script` passes `object_owner` to `generic_estate_permission`

**Severity**: LOW
**File**: `checks.rs` line 404

**Description**: The fallback at the end of `can_run_script` calls
`self.generic_estate_permission(object_owner)` instead of
`self.generic_estate_permission(agent_id)`. This is consistent with the C#
implementation, which checks whether the **object's owner** (not the requesting
agent) has estate privileges. However, this means:

1. A script owned by an estate manager runs anywhere, even if the requesting
   agent is a stranger.
2. A script owned by a stranger does not get the estate fallback, even if the
   requesting agent is an estate manager.

This behaviour is intentional in C# (scripts inherit their owner's privileges)
but is worth flagging as it may be non-obvious.

**Recommended fix**: Add a doc comment explaining the design intent.

### L-3. `ObjectPerms` struct defined but unused

**Severity**: LOW
**File**: `checks.rs` lines 20-27

**Description**: The `ObjectPerms` struct is defined and exported but no check
function accepts it. The check functions take individual parameters (`object_owner`,
`object_group`, `object_perms`) instead. This is dead code that suggests an
incomplete refactoring.

**Recommended fix**: Either refactor the check functions to use `ObjectPerms` (and
extend it per M-3) or remove the struct.

---

## Informational

### I-1. `implicit_gods` and `automatic_gods` interaction

The `initialise()` method (line 271-273) correctly enforces that `implicit_gods = true`
forces `automatic_gods = false`. However, there is no reverse enforcement:
`automatic_gods = true` does not force `implicit_gods = false`. This is consistent
with C# but means the two flags are not truly mutual exclusives — `implicit_gods`
has priority. This asymmetry is documented only in code, not in user-facing config
documentation.

### I-2. `from_bits_truncate` silently drops unknown parcel flags

`ParcelFlags::from_bits_truncate(parcel.flags)` (used in `can_create_object` line 90,
`can_run_script` line 379) silently discards any bits not defined in the `ParcelFlags`
bitflags. If future protocol versions add new flag bits, they will be silently ignored
rather than causing an error. This is standard bitflags behaviour but should be noted
for forward-compatibility planning.

### I-3. Missing permission checks vs. C# `DefaultPermissionsModule`

The following C# permission check methods have no Rust equivalent:

| C# Method | Risk |
|---|---|
| `CanInstantMessage` | Low — IM permissions rarely restricted |
| `CanViewScript` / `CanEditScript` | Medium — `allowed_script_editors` is parsed but never checked |
| `CanViewNotecard` / `CanEditNotecard` | Low — notecards follow object perms |
| `CanResetScript` | Low — usually follows edit permission |
| `CanBuyLand` | Medium — land purchases unprotected |
| `CanLinkObject` / `CanDelinkObject` | Low — follows edit permission |
| `CanTeleport` | Low — typically always allowed |
| `CanControlPrimMedia` / `CanInteractWithPrimMedia` | Low — media controls |

The `allowed_script_editors` config value is parsed during `initialise()` (line 282-286)
but never referenced by any check function — `can_compile_script` only checks
`allowed_script_creators`. This is dead config.

### I-4. No logging of denied permission checks

Permission denials are silent. In a production system, denied actions should be logged
at `DEBUG` or `TRACE` level with the agent UUID, action, and reason for denial. This
aids incident investigation and helps grid operators diagnose configuration issues.

### I-5. `god_users` set is writable without access control

The `god_users` field is `pub` on `DefaultPermissionsModule`. Any code with a reference
to the module can add UUIDs to `god_users`, granting them unrestricted god-mode (per H-1,
without even needing `allow_grid_gods`). In a production system, modification of the god
list should go through a controlled API that logs changes and validates authorisation.

---

## Recommendations

1. **Before any multi-user deployment**: Fix C-1 (nil UUID bypass) and H-1 (god-mode
   escalation past `allow_grid_gods`). These are the only two findings that could allow
   an attacker to gain privileges they should not have.

2. **Before production deployment**: Fix H-2 (add explicit god-mode activation gate)
   and H-3 (change default to bypass off). Address M-4 (compile script bypass
   inconsistency).

3. **During group system implementation**: Address M-2 (group permission no-ops) and
   M-3 (single permission mask). These block correct multi-user group functionality.

4. **For defence in depth**: Restrict `god_users` visibility (I-5), add permission
   denial logging (I-4), implement the missing checks from I-3 as the corresponding
   features are ported.

5. **Testing**: Add unit tests for nil UUID paths through every check function.
   The existing test suite (tests 61-100) does not test nil agent IDs against nil
   object owners. Add tests for `is_god()` with `allow_grid_gods = false` and a
   UUID in `god_users` to confirm the H-1 fix.
