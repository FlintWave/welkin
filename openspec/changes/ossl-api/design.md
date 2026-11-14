# ossl-api: OSSL extension functions

## Summary

Expand the OSSL extension library in `opensim-script-vm` from 27 stubs to
the full 258 unique `os*` functions defined in OpenSimulator 0.9.3.0. Port
the threat-level configuration system from INI files, implement every
function whose dependencies are already available, and stub the rest with
clear markers for the subsystems they await.

Reference: `OSSL_Api.cs` (6,649 lines, ~290 public methods including
overloads across 258 unique function names).

## Scope

### In scope

- All 258 `os*` functions from `IOSSL_Api.cs`, registered in `ossl.rs`.
- Full implementations for functions with no external dependencies (pure
  math, string, draw-list, crypto, regex, time, simulator-info).
- Threat-level configuration: parse `[OSSL]` section from INI, including
  `Allow_FunctionName` per-function overrides with role-based access
  (`ESTATE_OWNER`, `ESTATE_MANAGER`, `PARCEL_OWNER`, etc.).
- `OsslConfig` expansion: per-function allow-lists with role checking,
  `osslParcelO`/`osslParcelOG`/`osslNPC` macro expansion.
- Deprecated-function aliases (`osTerrainSetHeight` -> `osSetTerrainHeight`,
  `osSunGetParam` -> `osGetSunParam`, etc.).
- Port OSSL tests from the three C# test files plus new Rust-native tests.
- Remove `osWeatherMap` (not in C# OSSL; was a placeholder in the spike).

### Out of scope (stubbed, implemented when dependencies land)

- **NPC module** (24 functions): `osNpcCreate`, `osNpcMoveTo`, etc.
  Stubs return nil UUID / no-op. Awaits the NPC region module.
- **Parcel/land module** (12 functions): `osParcelJoin`,
  `osSetParcelDetails`, `osGetParcelDwell`, etc. Awaits land management.
- **Attachment system** (11 functions): `osForceAttachToAvatar`,
  `osDropAttachment`, etc. Awaits attachment handling in scene objects.
- **Inventory integration** (17 functions): `osGetInventoryItemKey`,
  `osGiveLinkInventory`, etc. Awaits full inventory<->script bridge.
- **Teleport service** (7 functions): `osTeleportAgent`,
  `osLocalTeleportAgent`, `osTeleportObject`. Awaits teleport service.
- **Dynamic texture module** (7 functions): `osSetDynamicTextureURL` and
  variants. Awaits the dynamic texture region module.
- **Sound module** (11 functions): `osPlaySound`, `osLoopSound`, etc.
  Awaits the sound module interface.
- **Avatar animation** (2 functions): `osAvatarPlayAnimation`,
  `osAvatarStopAnimation`. Awaits animation system.
- **Environment module** (4 functions): `osReplaceAgentEnvironment`,
  `osReplaceRegionEnvironment`, etc. Awaits environment module.
- **Region services** for terrain (5 functions), region restart/notice (3),
  kick (2), groups (2), console (1), HTTP URLs (2), particles (2).

These stubs all follow the same pattern as the existing 27: log via
`tracing::debug!`, return a sensible default (empty string, nil UUID, 0,
`()` for void), and can be filled in with a single future change per module.

## Architecture

### Module structure

The existing `crates/opensim-script-vm/src/ossl.rs` stays as the single
module. It grows from ~700 lines to approximately 4,000-5,000 lines. If
that becomes unwieldy during implementation, the registration functions
split into sub-modules by area (e.g., `ossl/draw.rs`, `ossl/npc.rs`,
`ossl/string.rs`), re-exported from `ossl.rs`. The `OsslThreatLevel`,
`OsslConfig`, `register_ossl()`, and `function_catalog()` stay in the root.

```
crates/opensim-script-vm/src/
  ossl.rs              -- OsslThreatLevel, OsslConfig, register_ossl(), catalog
  ossl/
    config.rs          -- INI parsing, per-function allow-lists, role macros
    draw.rs            -- 24 draw-list functions (pure string, no context)
    string.rs          -- 17 string/format/regex functions
    math.rs            -- 12 math/utility functions
    crypto.rs          -- 5 crypto functions (SHA-256, AES)
    time.rs            -- 6 time functions
    sim_info.rs        -- 8 simulator/grid/physics info functions
    agent.rs           -- 19 agent/avatar/health functions (mostly stubs)
    npc.rs             -- 24 NPC functions (all stubs)
    teleport.rs        -- 7 teleport functions (all stubs)
    terrain.rs         -- 5 terrain functions (stubs)
    parcel.rs          -- 12 parcel/land functions (stubs)
    inventory.rs       -- 17 inventory functions (stubs)
    attachment.rs      -- 11 attachment functions (stubs)
    sound.rs           -- 11 sound functions (stubs)
    environment.rs     -- 10 sun/wind/environment functions (partial)
    link_prim.rs       -- 14 link/prim/object functions (stubs)
    notecard.rs        -- 5 notecard functions (stubs)
    misc.rs            -- remaining functions
```

### OsslConfig expansion

The current `OsslConfig` has two fields: `max_threat_level` and
`allowed_functions: HashSet<String>`. This expands to:

```rust
pub struct OsslConfig {
    pub os_functions_enabled: bool,
    pub max_threat_level: OsslThreatLevel,
    /// Per-function permission overrides.
    /// Key: function name (e.g. "osGetSimulatorVersion").
    /// Value: who may call it.
    pub function_perms: HashMap<String, FunctionPerm>,
    pub permission_error_to_owner: bool,
}

pub enum FunctionPerm {
    /// Disabled entirely.
    Disabled,
    /// Enabled for everyone.
    Everyone,
    /// Enabled for specific roles and/or UUIDs.
    AllowList(AllowedRoles),
}

pub struct AllowedRoles {
    pub roles: HashSet<OsslRole>,
    pub allowed_uuids: HashSet<Uuid>,
}

pub enum OsslRole {
    GridGod,
    God,
    ActiveGod,
    EstateManager,
    EstateOwner,
    ParcelOwner,
    ParcelGroupMember,
}
```

The `is_allowed()` method becomes context-aware: it takes the script owner
UUID, their role in the current context (estate, parcel), and checks both
the global threat level and per-function overrides.

### INI configuration loading

Parse `osslDefaultEnable.ini` and optional `osslEnable.ini` using the
existing INI parser (`configparser` or equivalent crate). The `[OSSL]`
section provides:

1. `AllowOSFunctions` (bool, default true)
2. `OSFunctionThreatLevel` (string -> `OsslThreatLevel`)
3. `Allow_<FunctionName>` entries: `true`, `false`, or comma-separated
   role/UUID list
4. Macro definitions: `osslParcelO`, `osslParcelOG`, `osslNPC` with
   `${OSSL|macroname}` expansion
5. `Creators__<FunctionName>` entries for creator-based access

### Integration with ScriptContext

Functions that need scene access use the existing `Arc<RwLock<ScriptContext>>`
pattern. The `ScriptContext` struct already provides `script_id`, `prim_id`,
and `owner_id`. Functions needing richer context (parcel info, estate role,
etc.) will query through service traits added to `ScriptContext` as they
become available.

## Function inventory

### Group 1: Draw-list functions (24 functions, all `None`)

Pure string manipulation. No side effects, no context needed.

| Function | Status | Notes |
|----------|--------|-------|
| osMovePen | NEW | `MoveTo x,y;` |
| osDrawLine | EXISTS | 2 overloads in C# |
| osDrawText | EXISTS | |
| osDrawEllipse | EXISTS | |
| osDrawFilledEllipse | NEW | |
| osDrawRectangle | EXISTS | |
| osDrawFilledRectangle | NEW | |
| osDrawPolygon | NEW | Takes x,y coordinate lists |
| osDrawFilledPolygon | NEW | |
| osDrawResetTransform | NEW | |
| osDrawRotationTransform | NEW | |
| osDrawScaleTransform | NEW | |
| osDrawTranslationTransform | NEW | |
| osSetFontName | NEW | |
| osSetFontSize | NEW | |
| osSetPenSize | NEW | |
| osSetPenColor | NEW | 3 overloads |
| osSetPenColour | NEW | Deprecated alias |
| osSetPenCap | NEW | |
| osDrawImage | EXISTS | |
| osGetDrawStringSize | NEW | Stub (needs font metrics) |

**Implementable now:** 23 of 24 (osGetDrawStringSize needs font metrics,
returns stub).

### Group 2: String and formatting functions (17 functions, VeryLow/Low)

| Function | Threat | Status | Notes |
|----------|--------|--------|-------|
| osFormatString | VeryLow | EXISTS | Implemented |
| osMatchString | VeryLow | NEW | Regex pattern matching |
| osReplaceString | VeryLow | NEW | Regex replace |
| osStringSubString | Low | NEW | 2 overloads |
| osStringStartsWith | Low | NEW | |
| osStringEndsWith | Low | NEW | |
| osStringIndexOf | Low | NEW | 2 overloads |
| osStringLastIndexOf | Low | NEW | 2 overloads |
| osStringRemove | Low | NEW | |
| osStringReplace | Low | NEW | |
| osRegexIsMatch | Low | NEW | |
| osListenRegex | Low | NEW | Stub (needs listen system) |
| osMakeHash | VeryLow | EXISTS | Implemented |
| osSHA256 | Low | NEW | Crypto hash |
| osKey2Name | Low | NEW | Stub (needs user service) |
| osAvatarName2Key | Low | NEW | Stub (needs user service) |
| osGetAvatarHomeURI | Low | NEW | Stub (needs HG service) |

**Implementable now:** 12 pure string/regex functions. 5 need service stubs.

### Group 3: Math and utility functions (12 functions, None/Low)

| Function | Threat | Status |
|----------|--------|--------|
| osMin | None | NEW |
| osMax | None | NEW |
| osIsUUID | None | NEW |
| osRound | None | NEW |
| osVecMagSquare | None | NEW |
| osVecDistSquare | None | NEW |
| osAngleBetween | None | NEW |
| osApproxEquals | None | NEW (6 overloads -> 3 Lua functions) |
| osIsNotValidNumber | None | NEW |
| osSlerp | None | NEW (quaternion + vector overloads) |
| osOldList2ListStrided | None | NEW |
| osListSortInPlace | None | NEW |
| osListSortInPlaceStrided | None | NEW |

**Implementable now:** All 12 (pure math).

### Group 4: Cryptographic functions (5 functions, Low/High)

| Function | Threat | Status |
|----------|--------|--------|
| osSHA256 | Low | NEW |
| osAESEncrypt | Severe | NEW |
| osAESDecrypt | Severe | NEW |
| osAESEncryptTo | Severe | NEW |
| osAESDecryptFrom | Severe | NEW |

**Implementable now:** All 5 (use `ring` or `aes` crate).

### Group 5: Time functions (6 functions, None)

| Function | Threat | Status |
|----------|--------|--------|
| osUnixTimeToTimestamp | None | NEW |
| osGetPSTWallclock | None | NEW |
| osGetApparentTime | None | NEW |
| osGetApparentTimeString | None | NEW |
| osGetApparentRegionTime | None | NEW |
| osGetApparentRegionTimeString | None | NEW |

**Implementable now:** All 6 (use `chrono` crate).

### Group 6: Simulator and grid info (8 functions, High/Moderate/None)

| Function | Threat | Status | Notes |
|----------|--------|--------|-------|
| osGetSimulatorVersion | High | EXISTS | Implemented |
| osGetScriptEngineName | High | NEW | Returns "opensim-rust-luau" |
| osCheckODE | None | NEW | Returns 0 (Rapier, not ODE) |
| osGetPhysicsEngineType | High | NEW | Returns "Rapier" |
| osGetPhysicsEngineName | High | NEW | Returns "Rapier" |
| osGetGridNick | Moderate | NEW | From grid config |
| osGetGridName | Moderate | NEW | From grid config |
| osGetGridLoginURI | Moderate | NEW | From grid config |
| osGetGridHomeURI | Moderate | NEW | From grid config |
| osGetGridGatekeeperURI | Moderate | NEW | From grid config |
| osGetGridCustom | Moderate | NEW | From grid config |
| osGetSimulatorMemory | Moderate | NEW | Process RSS |
| osGetSimulatorMemoryKB | Moderate | NEW | Process RSS |
| osGetRegionStats | Moderate | NEW | Stub (needs metrics) |
| osGetRegionSize | None | NEW | From region config |

**Implementable now:** 7 static-info functions. 6 grid-info functions
need config access (available via ScriptContext). 2 need runtime metrics.

### Group 7: NPC functions (24 functions, High/Low/None)

| Function | Threat | Notes |
|----------|--------|-------|
| osIsNpc | None | |
| osNpcCreate | High | 2 overloads |
| osNpcSaveAppearance | High | 2 overloads |
| osNpcLoadAppearance | High | |
| osNpcGetOwner | None | |
| osNpcGetPos | High | |
| osNpcMoveTo | High | |
| osNpcMoveToTarget | High | |
| osNpcGetRot | High | |
| osNpcSetRot | High | |
| osNpcStopMoveToTarget | High | |
| osNpcSetProfileAbout | Low | |
| osNpcSetProfileImage | Low | |
| osNpcSay | High | 2 overloads |
| osNpcSayTo | High | |
| osNpcShout | High | |
| osNpcSit | High | |
| osNpcStand | High | |
| osNpcRemove | High | |
| osNpcPlayAnimation | High | |
| osNpcStopAnimation | High | |
| osNpcTouch | High | |
| osNpcWhisper | High | |
| osNpcLookAt | High | |

**Implementable now:** 0. All stubs until the NPC region module lands.
The existing 4 stubs (osNpcCreate, osNpcRemove, osNpcMoveTo, osNpcSay)
expand to the full 24.

### Group 8: Teleport functions (7 functions, Severe/None)

| Function | Threat |
|----------|--------|
| osTeleportAgent | Severe (3 overloads) |
| osTeleportOwner | None (3 overloads) |
| osLocalTeleportAgent | Severe |
| osTeleportObject | Severe |

**Implementable now:** 0. All stubs. Existing 2 stubs expand to 4 unique
functions (7 overloads handled by Lua dispatch).

### Group 9: Agent and avatar functions (19 functions, various)

| Function | Threat | Notes |
|----------|--------|-------|
| osGetAgents | None | Stub (needs presence list) |
| osGetAgentIP | Severe | Stub (admin-only) |
| osGetAvatarList | None | Stub |
| osGetNPCList | None | Stub |
| osSetSpeed | Moderate | EXISTS (stub) |
| osGetSpeed | Moderate | EXISTS (stub) |
| osSetOwnerSpeed | Moderate | NEW stub |
| osGetHealth | None | NEW stub |
| osCauseHealing | High | NEW stub |
| osCauseDamage | High | NEW stub |
| osSetHealth | High | NEW stub |
| osSetHealRate | High | NEW stub |
| osGetHealRate | None | NEW stub |
| osKickAvatar | Severe | NEW stub (2 overloads) |
| osForceOtherSit | VeryHigh | NEW stub (2 overloads) |
| osAvatarType | None | NEW stub |
| osGetGender | None | NEW stub |
| osDetectedCountry | Moderate | NEW stub |
| osGetAgentCountry | Moderate | NEW stub |

**Implementable now:** 0 beyond existing stubs. All need presence/avatar
services.

### Group 10: Terrain functions (5 functions, High/VeryLow)

| Function | Threat |
|----------|--------|
| osSetTerrainHeight | High |
| osGetTerrainHeight | High |
| osTerrainSetHeight | High (deprecated alias) |
| osTerrainGetHeight | High (deprecated alias) |
| osTerrainFlush | VeryLow |

**Implementable now:** 0. Stubs until terrain module.

### Group 11: Environment and sun functions (10 functions, various)

| Function | Threat | Status |
|----------|--------|--------|
| osGetCurrentSunHour | None | EXISTS (stub) |
| osGetSunParam | None | NEW |
| osSunGetParam | None | Deprecated alias |
| osSetSunParam | None | EXISTS (stub) |
| osSunSetParam | None | Deprecated alias |
| osSetRegionWaterHeight | High | EXISTS (stub) |
| osSetRegionSunSettings | High | NEW stub |
| osSetEstateSunSettings | Nuisance | NEW stub |
| osWindActiveModelPluginName | None | NEW stub |
| osSetWindParam | VeryLow | NEW stub |
| osGetWindParam | VeryLow | NEW stub |
| osReplaceAgentEnvironment | Moderate | NEW stub |
| osReplaceParcelEnvironment | Moderate | NEW stub |
| osReplaceRegionEnvironment | Moderate | NEW stub |
| osResetEnvironment | Moderate | NEW stub |

**Implementable now:** 2 (sun param read/write are pure config).
13 stubs.

### Group 12: Parcel and land functions (12 functions, High/Moderate/None)

| Function | Threat |
|----------|--------|
| osGetParcelDwell | None |
| osParcelJoin | High |
| osParcelSubdivide | High |
| osSetParcelDetails | VeryHigh |
| osParcelSetDetails | VeryHigh (deprecated) |
| osGetParcelDetails | None |
| osGetParcelIDs | None |
| osGetParcelID | None |
| osSetParcelMusicURL | VeryLow |
| osSetParcelMediaURL | VeryLow |
| osSetParcelSIPAddress | VeryLow |
| osSetPrimFloatOnWater | VeryLow |

**Implementable now:** 0. All stubs until land module.

### Group 13: Inventory functions (17 functions, various)

| Function | Threat |
|----------|--------|
| osGetInventoryLastOwner | None |
| osGetInventoryItemKey | None |
| osGetInventoryName | None |
| osGetInventoryDesc | None |
| osGetInventoryItemKeys | None |
| osGetInventoryNames | None |
| osGetLinkInventoryKey | None |
| osGetLinkInventoryKeys | None |
| osGetLinkInventoryItemKey | None |
| osGetLinkInventoryName | None |
| osGetLinkInventoryDesc | None |
| osGetLinkInventoryItemKeys | None |
| osGetLinkInventoryNames | None |
| osRemoveLinkInventory | None |
| osGiveLinkInventory | None |
| osGiveLinkInventoryList | None |
| osGetLastChangedEventKey | None |

**Implementable now:** 0. All stubs until inventory-script bridge.

### Group 14: Attachment functions (11 functions, High/VeryHigh/Moderate)

| Function | Threat |
|----------|--------|
| osForceAttachToAvatar | High |
| osForceAttachToAvatarFromInventory | High |
| osForceAttachToOtherAvatarFromInventory | VeryHigh |
| osForceDetachFromAvatar | VeryHigh |
| osGetNumberOfAttachments | Moderate |
| osMessageAttachments | Moderate |
| osDropAttachment | Moderate |
| osForceDropAttachment | High |
| osDropAttachmentAt | Moderate |
| osForceDropAttachmentAt | High |
| osCollisionSound | VeryLow |

**Implementable now:** 0. All stubs until attachment system.

### Group 15: Sound functions (11 functions, Low)

| Function | Threat |
|----------|--------|
| osAdjustSoundVolume | Low |
| osLoopSound | Low |
| osLoopSoundMaster | Low |
| osLoopSoundSlave | Low |
| osPlaySound | Low |
| osPlaySoundSlave | Low |
| osPreloadSound | Low |
| osSetSoundRadius | Low |
| osStopSound | Low |
| osTriggerSound | Low |
| osTriggerSoundLimited | Low |

**Implementable now:** 0. All stubs until sound module.

### Group 16: Link, prim, and object functions (14 functions, various)

| Function | Threat |
|----------|--------|
| osGetLinkPrimitiveParams | High |
| osGetPrimitiveParams | High |
| osSetPrimitiveParams | High |
| osSetProjectionParams | High (3 overloads) |
| osForceCreateLink | VeryLow |
| osForceBreakLink | VeryLow |
| osForceBreakAllLinks | VeryLow |
| osDie | Low |
| osGetLinkNumber | None |
| osMessageObject | Low |
| osGetRezzingObject | None |
| osSetContentType | Severe |
| osVolumeDetect | VeryLow |
| osGetPrimCount | None (2 overloads) |
| osGetSittingAvatarsCount | None (2 overloads) |

**Implementable now:** 0. All stubs until scene object services.

### Group 17: Notecard functions (5 functions, High/VeryHigh)

| Function | Threat | Status |
|----------|--------|--------|
| osGetNotecard | VeryHigh | EXISTS (stub) |
| osGetNotecardLine | VeryHigh | EXISTS (stub) |
| osGetNumberOfNotecardLines | VeryHigh | EXISTS (stub) |
| osMakeNotecard | High | EXISTS (stub) |
| osLoadedCreationDate | Low | NEW stub |
| osLoadedCreationTime | Low | NEW stub |
| osLoadedCreationID | Low | NEW stub |

**Implementable now:** 0. All stubs until inventory/notecard service.

### Group 18: Appearance and map functions (7 functions, various)

| Function | Threat |
|----------|--------|
| osOwnerSaveAppearance | High (2 overloads) |
| osAgentSaveAppearance | VeryHigh (2 overloads) |
| osGetGender | None |
| osGetMapTexture | None |
| osGetRegionMapTexture | High |
| osAvatarPlayAnimation | VeryHigh |
| osAvatarStopAnimation | VeryHigh |

**Implementable now:** 0. All stubs.

### Group 19: Inertia and physics functions (6 functions, various)

| Function | Threat |
|----------|--------|
| osGetInertiaData | None |
| osClearInertia | None |
| osSetInertia | None |
| osSetInertiaAsBox | None |
| osSetInertiaAsSphere | None |
| osSetInertiaAsCylinder | None |

**Implementable now:** 0. Stubs until physics module integration.

### Group 20: Sit/stand, particle, group, misc (15 functions, various)

| Function | Threat |
|----------|--------|
| osSetSitActiveRange | None |
| osSetLinkSitActiveRange | None |
| osGetSitActiveRange | None |
| osGetLinkSitActiveRange | None |
| osGetSitTargetPos | None |
| osGetSitTargetRot | None |
| osSetStandTarget | None |
| osSetLinkStandTarget | None |
| osGetStandTarget | None |
| osGetLinkStandTarget | None |
| osClearObjectAnimations | None |
| osResetAllScripts | None |
| osParticleSystem | None |
| osLinkParticleSystem | None |
| osInviteToGroup | VeryLow |
| osEjectFromGroup | VeryLow |
| osConsoleCommand | Severe |
| osRequestURL | Moderate |
| osRequestSecureURL | Moderate |
| osSetRot | VeryHigh |
| osGetLinkColor | None |
| osTemperature2sRGB | None |

**Implementable now:** 1 (osTemperature2sRGB is pure math).
osGetLinkColor needs scene. Rest are stubs.

### Summary

| Category | Functions | Implementable now | Stubs |
|----------|-----------|-------------------|-------|
| Draw-list | 24 | 23 | 1 |
| String/format/regex | 17 | 12 | 5 |
| Math/utility | 12 | 12 | 0 |
| Crypto | 5 | 5 | 0 |
| Time | 6 | 6 | 0 |
| Sim/grid info | 15 | 9 | 6 |
| NPC | 24 | 0 | 24 |
| Teleport | 4 | 0 | 4 |
| Agent/avatar | 19 | 0 | 19 |
| Terrain | 5 | 0 | 5 |
| Environment/sun/wind | 15 | 2 | 13 |
| Parcel/land | 12 | 0 | 12 |
| Inventory | 17 | 0 | 17 |
| Attachment | 11 | 0 | 11 |
| Sound | 11 | 0 | 11 |
| Link/prim/object | 15 | 0 | 15 |
| Notecard | 7 | 0 | 7 |
| Appearance/map | 7 | 0 | 7 |
| Inertia/physics | 6 | 0 | 6 |
| Sit/stand/misc | 22 | 2 | 20 |
| **Total** | **258** | **71** | **187** |

Approximately 71 functions (28%) can be fully implemented. The remaining
187 (72%) are stubs that log and return defaults, exactly matching the
pattern already established in the existing 27 functions.

## Dependencies

### Already available

- `opensim-script-vm` crate with `ScriptContext`, `mlua`, `parking_lot`,
  `uuid`
- `tracing` for debug logging
- INI configuration parsing (region config system)
- Existing `OsslThreatLevel` enum and `OsslConfig` struct
- `register_ossl()` entry point and `function_catalog()` introspection

### New crate dependencies

- `sha2` for `osSHA256` (or use `ring`)
- `aes` + `cbc` for AES-128/256 in `osAESEncrypt`/`osAESDecrypt`
- `base64` for AES functions (already likely in tree)
- `chrono` for time functions (PST wallclock, apparent time)
- `regex` for `osMatchString`, `osReplaceString`, `osRegexIsMatch`

### Not yet available (stubs)

- **NPC module** -- needed for 24 functions
- **Land/parcel module** -- needed for 12 functions
- **Inventory<->script bridge** -- needed for 17 functions
- **Attachment system** -- needed for 11 functions
- **Sound module** -- needed for 11 functions
- **Teleport service** -- needed for 7 functions
- **Dynamic texture module** -- needed for 7 functions
- **Terrain module** -- needed for 5 functions
- **Environment module** -- needed for 4 functions
- **Avatar animation system** -- needed for 2 functions
- **Scene object services** (for link/prim) -- needed for 15 functions
- **User/presence services** -- needed for agent functions

## Testing strategy

### Unit tests (in `ossl.rs` and sub-modules)

1. **All 71 implemented functions** get at least one positive test each,
   verifying correct output for known inputs.
2. **Draw-list chaining** tests: build a multi-step draw list and verify
   the concatenated command string.
3. **String function** tests: edge cases for regex, empty strings, Unicode,
   out-of-bounds indices.
4. **Math function** tests: NaN handling, infinity, precision for
   `osApproxEquals`.
5. **Crypto function** tests: known-answer tests for SHA-256 and AES
   round-trip (encrypt then decrypt recovers plaintext).
6. **Time function** tests: verify timestamp formatting.

### Threat-level tests

7. **INI parsing** tests: load a test INI string, verify `OsslConfig`
   fields including macro expansion and per-function overrides.
8. **Role-based access** tests: verify that functions gated to
   `ESTATE_MANAGER` are blocked for regular users and allowed for managers.
9. **Regression**: the existing 12 tests continue to pass.

### Ported from C# test files

10. `OSSL_ApiNpcTests.cs`: NPC creation, removal, movement (adapted to
    verify stubs return expected defaults without crashing).
11. `OSSL_ApiAttachmentTests.cs`: attachment force-attach, detach (adapted
    as above).
12. `OSSL_ApiAppearanceTest.cs`: appearance save/load (adapted as above).

### Integration-ready tests

13. Each stub function group has a test that registers all functions at
    `Severe` level and calls each one, verifying no panics and that return
    values match expected defaults.

### Second review

14. Adversarial review of threat-level enforcement. Verify that:
    - No function bypasses CheckThreatLevel.
    - Per-function overrides cannot be injected by script content.
    - The INI parser rejects malformed entries without crashing.
    - AES functions do not leak key material through errors.
