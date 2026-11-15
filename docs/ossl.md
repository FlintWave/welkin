<!-- SPDX-License-Identifier: LGPL-3.0-or-later -->

# OSSL function reference

[← Back to docs](README.md) · [Script engine](scripting.md) · [Architecture](architecture.md) · [Security review](security-review-ossl-api.md)

OSSL (OpenSim Scripting Language) is the set of `os*` extension functions that
OpenSimulator adds on top of standard LSL. They give scripts access to
simulator-specific features: NPC control, dynamic textures, terrain editing,
teleportation, sound, attachments, and region administration. Welkin implements
these for parity with OpenSimulator 0.9.3.0.

The C# reference (`OSSL_Api.cs`) defines 258 unique function names. Welkin
registers 236 of those. The remaining 22 are deprecated aliases and overloads
that the C# engine keeps for backward compatibility; Welkin folds them into the
canonical registration.

Source: `crates/welkin-script-vm/src/ossl/` (13 files).

## Threat levels

Every OSSL function carries a threat level that controls whether a region
permits its use. The system mirrors the C# `ThreatLevel` enum:

| Level | Value | Meaning |
|---|---|---|
| None | 0 | No side effects |
| Nuisance | 1 | Abuse can cause log spam |
| VeryLow | 2 | Extreme abuse can impair a region slightly |
| Low | 3 | Intentional abuse can cause recoverable crashes |
| Moderate | 4 | Intentional abuse can cause denial of service with potential data loss |
| High | 5 | Casual abuse can impair functionality; intentional abuse causes data loss |
| VeryHigh | 6 | Normal use may cause service impairment depending on frequency |
| Severe | 7 | Even casual use can endanger region stability |

The region's `max_threat_level` setting (default: VeryLow) determines the
ceiling. Functions above that ceiling are not registered in the Luau VM at
all, so scripts can't reach them. This is stronger than the C# approach, which
checks at call time and can be bypassed by reflection.

### Per-function overrides

Individual functions can be allowed or restricted regardless of the global
ceiling. In the `[OSSL]` section of the region config:

```ini
[OSSL]
OSFunctionThreatLevel = VeryLow

; Allow a specific function above the ceiling
Allow_osGetSimulatorVersion = true

; Restrict a function to specific roles
Allow_osNpcCreate = ESTATE_MANAGER,ESTATE_OWNER

; Use macros for common role sets
Allow_osNpcRemove = ${OSSL|osslParcelOG}
```

Available macros:

| Macro | Expands to |
|---|---|
| `${OSSL\|osslParcelO}` | PARCEL_OWNER |
| `${OSSL\|osslParcelOG}` | PARCEL_OWNER, PARCEL_GROUP_MEMBER |
| `${OSSL\|osslNPC}` | ESTATE_MANAGER, ESTATE_OWNER |
| `${OSSL\|osslParcelP}` | PARCEL_OWNER, PARCEL_GROUP_MEMBER, ESTATE_MANAGER, ESTATE_OWNER |
| `${OSSL\|osslEstateO}` | ESTATE_OWNER |
| `${OSSL\|osslEstateM}` | ESTATE_MANAGER |

Roles checked: Anyone, EstateManager, EstateOwner, ParcelOwner,
ParcelGroupMember.

> [!NOTE]
> Role-based permission checks run at registration time, not call time. Per
> the [security review](security-review-ossl-api.md) finding S2, this means
> role restrictions are not enforced per-caller until a caller context is added
> to `ScriptContext`. For now, treat role-restricted functions as
> threat-level-gated only.

## Functions by category

### NPC control (24 functions, High)

Create, move, animate, and remove non-player characters.

`osNpcCreate`, `osNpcRemove`, `osNpcSaveAppearance`, `osNpcLoadAppearance`,
`osNpcGetOwner`, `osNpcGetPos`, `osNpcGetRot`, `osNpcSetRot`,
`osNpcMoveTo`, `osNpcMoveToTarget`, `osNpcStopMoveToTarget`,
`osNpcLookAt`, `osNpcSay`, `osNpcSayTo`, `osNpcShout`, `osNpcWhisper`,
`osNpcSit`, `osNpcStand`, `osNpcPlayAnimation`, `osNpcStopAnimation`,
`osNpcTouch`, `osNpcSetProfileAbout`, `osNpcSetProfileImage`, `osIsNpc`

### Agent and avatar (20 functions, Moderate to Severe)

Health, damage, teleportation, animation, and presence queries.

`osGetAgents`, `osGetAvatarList`, `osGetNPCList`, `osSetOwnerSpeed`,
`osGetHealth`, `osCauseHealing`, `osCauseDamage`, `osSetHealth`,
`osSetHealRate`, `osGetHealRate`, `osKickAvatar`, `osForceOtherSit`,
`osAvatarType`, `osGetGender`, `osDetectedCountry`, `osGetAgentCountry`,
`osAvatarPlayAnimation`, `osAvatarStopAnimation`, `osLocalTeleportAgent`,
`osTeleportObject`

### Drawing (21 functions, None)

Build draw-list command strings for dynamic textures. Pure string
manipulation with no side effects.

`osDrawText`, `osDrawLine`, `osDrawRectangle`, `osDrawEllipse`,
`osDrawImage`, `osMovePen`, `osDrawFilledEllipse`, `osDrawFilledRectangle`,
`osDrawPolygon`, `osDrawFilledPolygon`, `osDrawResetTransform`,
`osDrawRotationTransform`, `osDrawScaleTransform`,
`osDrawTranslationTransform`, `osSetFontName`, `osSetFontSize`,
`osSetPenSize`, `osSetPenColor`, `osSetPenColour`, `osSetPenCap`,
`osGetDrawStringSize`

### String and regex (10 functions, VeryLow)

Pattern matching and string manipulation beyond what LSL provides.

`osMatchString`, `osReplaceString`, `osStringSubString`,
`osStringStartsWith`, `osStringEndsWith`, `osStringIndexOf`,
`osStringLastIndexOf`, `osStringRemove`, `osStringReplace`,
`osRegexIsMatch`

### Math and list utilities (13 functions, None)

Vector math, rotation helpers, list sorting.

`osMin`, `osMax`, `osIsUUID`, `osRound`, `osVecMagSquare`,
`osVecDistSquare`, `osAngleBetween`, `osApproxEquals`,
`osIsNotValidNumber`, `osSlerp`, `osOldList2ListStrided`,
`osListSortInPlace`, `osListSortInPlaceStrided`

### Cryptography (5 functions, Low to VeryLow)

SHA-256 hashing and AES-256-CBC encryption with PBKDF2 key derivation. Key
derivation uses HMAC-SHA1 with 1000 iterations and a fixed salt
(`OpenSimulator OSSL`) to match .NET `Rfc2898DeriveBytes`.

`osSHA256`, `osAESEncrypt`, `osAESDecrypt`, `osAESEncryptTo`,
`osAESDecryptFrom`

> [!WARNING]
> The security review flagged that PBKDF2 was initially using SHA-256 instead
> of SHA-1. This has been corrected, but a C#-generated test vector is still
> needed to confirm interoperability. See finding S1 in the
> [security review](security-review-ossl-api.md).

### Time (6 functions, None)

Wall-clock and apparent (sim-time) queries.

`osUnixTimeToTimestamp`, `osGetPSTWallclock`, `osGetApparentTime`,
`osGetApparentTimeString`, `osGetApparentRegionTime`,
`osGetApparentRegionTimeString`

### Simulator and grid info (16 functions, Moderate to High)

Queries about the running simulator, physics engine, grid topology, and
region statistics.

`osGetScriptEngineName`, `osCheckODE`, `osGetPhysicsEngineType`,
`osGetPhysicsEngineName`, `osGetRegionSize`, `osGetSimulatorMemory`,
`osGetSimulatorMemoryKB`, `osTemperature2sRGB`, `osGetGridNick`,
`osGetGridName`, `osGetGridLoginURI`, `osGetGridHomeURI`,
`osGetGridGatekeeperURI`, `osGetGridCustom`, `osGetRegionStats`,
`osGetMapTexture`

### Terrain and environment (19 functions, Moderate to VeryHigh)

Read and write terrain heightmaps, water level, sun, wind, and parcel
properties.

`osTerrainSetHeight`, `osSetTerrainHeight`, `osTerrainGetHeight`,
`osGetTerrainHeight`, `osTerrainFlush`, `osSunGetParam`,
`osSetEstateSunSettings`, `osSetRegionSunSettings`, `osGetWindParam`,
`osSetWindParam`, `osGetRegionWaterHeight`, `osParcelSetDetails`,
`osGetParcelDetails`, `osGetParcelDwell`, `osGetParcelPrimCount`,
`osSetParcelDetails`, `osSetParcelMusicURL`, `osSetParcelSIPAddress`,
`osSetPrimFloatOnWater`

### Inventory, attachments, and sound (51 functions, VeryLow to VeryHigh)

Inventory queries and transfers, forced attachment operations, sound
playback, particle effects, object linking, and object management.

`osGiveInventory`, `osGiveInventoryList`, `osForceDropAttachment`,
`osForceDropAttachmentAt`, `osGetInventoryDesc`, `osGetInventoryItemKey`,
`osGetInventoryLastOwner`, `osGetInventoryName`, `osGetInventoryNumber`,
`osGetInventoryPermMask`, `osSetInventoryDesc`, `osRemoveInventory`,
`osGetLastChangedEventKey`, `osTakeObject`, `osOwnerDropAttachment`,
`osOwnerDropAttachmentAt`, `osDropAttachment`, `osForceAttachToAvatar`,
`osForceAttachToAvatarAt`, `osForceAttachToOtherAvatarFromInventory`,
`osForceDetachFromAvatar`, `osGetNumberOfAttachments`,
`osMessageAttachments`, `osPlaySound`, `osLoopSound`, `osLoopSoundMaster`,
`osLoopSoundSlave`, `osStopSound`, `osPreloadSound`,
`osAdjustSoundVolume`, `osTriggerSound`, `osTriggerSoundLimited`,
`osSoundSetRadius`, `osMakeFire`, `osMakeSmoke`, `osMakeExplosion`,
`osForceCreateLink`, `osForceBreakLink`, `osForceBreakAllLinks`, `osDie`,
`osMessageObject`, `osGetRezzingObject`, `osSetContentType`,
`osVolumeDetect`, `osGetPrimCount`, `osGetSittingAvatarsCount`,
`osGetSittingAvatars`, `osSetProjectionParams`,
`osGetLinkPrimitiveParams`, `osSetPrimitiveParams`, `osGetLinkNumber`

### Notecard, textures, teleport, and region (23 functions, VeryLow to Severe)

Notecard reading and creation, dynamic texture rendering, teleportation,
speed control, region management, and a few string/hash utilities that
shipped with the original stub set.

`osGetNotecard`, `osGetNotecardLine`, `osGetNumberOfNotecardLines`,
`osMakeNotecard`, `osSetDynamicTextureURL`, `osSetDynamicTextureData`,
`osSetParcelMediaURL`, `osGetAgentIP`, `osTeleportAgent`,
`osTeleportOwner`, `osRegionRestart`, `osSetSpeed`, `osGetSpeed`,
`osGetSimulatorVersion`, `osGetCurrentSunHour`, `osSetSunParam`,
`osSetRegionWaterHeight`, `osRegionNotice`, `osSetTerrainTexture`,
`osSetTerrainTextureHeight`, `osGetSunParam`, `osFormatString`,
`osMakeHash`

### Miscellaneous (28 functions, None to Severe)

Region load metadata, appearance saving, inertia, particles, groups, HTTP
URLs, rotation, color, listener regex, and avatar resolution.

`osLoadedCreationDate`, `osLoadedCreationTime`, `osLoadedCreationID`,
`osOwnerSaveAppearance`, `osAgentSaveAppearance`,
`osGetRegionMapTexture`, `osGetInertiaData`, `osSetInertia`,
`osSetInertiaAsBox`, `osSetInertiaAsSphere`, `osSetInertiaAsCylinder`,
`osClearInertia`, `osSitOnLink`, `osClearObjectAnimations`,
`osResetAllScripts`, `osParticleSystem`, `osLinkParticleSystem`,
`osInviteToGroup`, `osEjectFromGroup`, `osConsoleCommand`,
`osRequestURL`, `osRequestSecureURL`, `osSetRot`, `osGetLinkColor`,
`osKey2Name`, `osAvatarName2Key`, `osGetAvatarHomeURI`, `osListenRegex`

## Threat level distribution

| Level | Count |
|---|---|
| None | ~96 |
| Nuisance | 0 |
| VeryLow | ~29 |
| Low | ~22 |
| Moderate | ~17 |
| High | ~42 |
| VeryHigh | ~23 |
| Severe | ~9 |
| **Total** | **236** |

Most None-level functions are draw-list builders, math helpers, and time
queries. The High and VeryHigh clusters are NPC control and avatar/object
manipulation. The nine Severe functions are `osGetAgentIP`,
`osRegionRestart`, `osConsoleCommand`, and similar operations that could
directly compromise privacy or availability.

## Implementation status

All 236 functions are registered in the Luau VM when their threat level
permits. Some execute real logic (draw functions produce draw-list strings,
crypto functions perform actual AES operations, terrain functions read and
write the heightmap, time functions return real clock values). Others are
stubs that log the call and return a neutral default (empty string, nil
UUID, zero). Each stub is filled in when the subsystem it depends on
is delivered.

Functions with real implementations: draw (21), string (10), math (13),
crypto (5), time (6), sim/grid info (16), terrain get/set (7), speed
get/set (2), sun/wind get/set (6), NPC core (8), health/damage (6), and
several others.

Functions that remain stubs: most inventory transfer operations, forced
attachment operations, sound playback, particle effects, dynamic texture
rendering, and `osGetDrawStringSize` (needs font metrics).

## Testing

The OSSL module has 29 unit tests covering:

- Threat-level ordering and configuration
- Registration gating at each threat level
- Per-function permission with role overrides
- INI parsing with macro expansion
- Draw-list chaining
- Crypto round-trip (AES encrypt/decrypt)
- Terrain heightmap read/write
- Catalog completeness (no duplicate names, minimum 160 functions)

Run them with:

```sh
cargo test -p welkin-script-vm ossl
```

## Commits

| Hash | Description |
|---|---|
| `9395d20` | OSSL framework: threat levels, config, INI parsing, draw/string/math/crypto/time/sim-info |
| `6b03016` | 24 NPC stubs, 20 agent functions |
| `d9698d5` | 19 terrain/environment/parcel functions |
| `cc0e50a` | 51 inventory/attachment/sound/link functions |
| `c8370df` | 28 misc functions (inertia, particles, groups, console, HTTP URLs) |

## See also

- [Script engine](scripting.md) for the full LSL pipeline and ll* builtins
- [OSSL API phase summary](ossl-api-summary.md) for the build history
- [Security review](security-review-ossl-api.md) for tracked findings
- [Viewer compatibility](viewer-compatibility.md) for end-to-end scripting verification
