## 1. Configuration and infrastructure

- [ ] 1.1 Expand `OsslConfig` with per-function `FunctionPerm` map, `OsslRole` enum, and context-aware `is_allowed()` (owner UUID, estate role, parcel ownership)
- [ ] 1.2 INI parser for `[OSSL]` section: `OSFunctionThreatLevel`, `Allow_<FunctionName>` entries with role lists, macro expansion (`${OSSL|osslParcelO}` etc.), `Creators__<FunctionName>` lists; tests for macro expansion and malformed input rejection
- [ ] 1.3 Split `ossl.rs` into sub-module files by area (draw, string, math, crypto, time, sim_info, npc, stubs); re-export from `ossl.rs`; remove `osWeatherMap`; update `function_catalog()` to cover all 258 functions

## 2. Fully implemented functions: draw-list (24 functions, threat None)

- [ ] 2.1 Implement 19 missing draw-list functions (`osMovePen`, `osDrawFilledEllipse`, `osDrawFilledRectangle`, `osDrawPolygon`, `osDrawFilledPolygon`, `osDrawResetTransform`, `osDrawRotationTransform`, `osDrawScaleTransform`, `osDrawTranslationTransform`, `osSetFontName`, `osSetFontSize`, `osSetPenSize`, `osSetPenColor` with 3 overloads, `osSetPenColour` deprecated alias, `osSetPenCap`, `osGetDrawStringSize` stub); tests for chaining and color parsing

## 3. Fully implemented functions: string and regex (12 functions)

- [ ] 3.1 Implement `osMatchString`, `osReplaceString`, `osStringSubString`, `osStringStartsWith`, `osStringEndsWith`, `osStringIndexOf`, `osStringLastIndexOf`, `osStringRemove`, `osStringReplace`, `osRegexIsMatch`; verify existing `osFormatString`/`osMakeHash` match C# behaviour; tests for regex edge cases, Unicode, out-of-bounds indices

## 4. Fully implemented functions: math and utility (12 functions, threat None)

- [ ] 4.1 Implement `osMin`, `osMax`, `osIsUUID`, `osRound`, `osVecMagSquare`, `osVecDistSquare`, `osAngleBetween`, `osApproxEquals` (float/vector/rotation with optional margin), `osIsNotValidNumber`, `osSlerp` (quaternion + vector), `osOldList2ListStrided`, `osListSortInPlace`, `osListSortInPlaceStrided`; tests for NaN/infinity, precision, quaternion slerp

## 5. Fully implemented functions: crypto (5 functions)

- [ ] 5.1 Implement `osSHA256` (SHA-256 hex digest), `osAESEncrypt`/`osAESDecrypt` (AES-256-CBC, PKCS7, base64 I/O), `osAESEncryptTo`/`osAESDecryptFrom` (explicit IV); tests with known-answer vectors and encrypt-decrypt round-trip

## 6. Fully implemented functions: time (6 functions, threat None)

- [ ] 6.1 Implement `osUnixTimeToTimestamp`, `osGetPSTWallclock`, `osGetApparentTime`, `osGetApparentTimeString`, `osGetApparentRegionTime`, `osGetApparentRegionTimeString`; tests for PST conversion and 12h/24h format

## 7. Fully implemented functions: sim/grid/physics info (15 functions)

- [ ] 7.1 Implement static-info functions (`osGetScriptEngineName`, `osCheckODE`, `osGetPhysicsEngineType`, `osGetPhysicsEngineName`, `osGetRegionSize`, `osGetSimulatorMemory`, `osGetSimulatorMemoryKB`, `osTemperature2sRGB`), grid-info from config (`osGetGridNick`, `osGetGridName`, `osGetGridLoginURI`, `osGetGridHomeURI`, `osGetGridGatekeeperURI`, `osGetGridCustom`), and stubs for `osGetRegionStats`/`osGetMapTexture`; tests for engine-name strings and grid-config reads

## 8. NPC function stubs (24 functions)

- [ ] 8.1 Expand existing 4 NPC stubs to full 24: add `osIsNpc`, `osNpcSaveAppearance`, `osNpcLoadAppearance`, `osNpcGetOwner`, `osNpcGetPos`, `osNpcMoveToTarget`, `osNpcGetRot`, `osNpcSetRot`, `osNpcStopMoveToTarget`, `osNpcSetProfileAbout`, `osNpcSetProfileImage`, `osNpcSayTo`, `osNpcShout`, `osNpcSit`, `osNpcStand`, `osNpcPlayAnimation`, `osNpcStopAnimation`, `osNpcTouch`, `osNpcWhisper`, `osNpcLookAt`; tests verify no panics and expected defaults

## 9. Agent, avatar, and teleport stubs (26 functions)

- [ ] 9.1 Register 19 agent/avatar stubs (`osGetAgents`, `osGetAvatarList`, `osGetNPCList`, `osSetOwnerSpeed`, `osGetHealth`, `osCauseHealing`, `osCauseDamage`, `osSetHealth`, `osSetHealRate`, `osGetHealRate`, `osKickAvatar`, `osForceOtherSit`, `osAvatarType`, `osGetGender`, `osDetectedCountry`, `osGetAgentCountry`, `osAvatarPlayAnimation`, `osAvatarStopAnimation`, `osGetAgentIP`) and 7 teleport stubs (`osTeleportAgent` overloads, `osTeleportOwner` overloads, `osLocalTeleportAgent`, `osTeleportObject`); tests verify defaults

## 10. Terrain, environment, wind, and parcel stubs (32 functions)

- [ ] 10.1 Register 5 terrain stubs, 12 environment/sun/wind stubs, 12 parcel/land stubs (including deprecated aliases `osTerrainSetHeight`, `osTerrainGetHeight`, `osSunGetParam`, `osSunSetParam`, `osParcelSetDetails`) and media stubs (`osSetParcelMusicURL`, `osSetParcelSIPAddress`, `osSetPrimFloatOnWater`); tests verify defaults

## 11. Inventory, attachment, sound, and link/prim stubs (54 functions)

- [ ] 11.1 Register 17 inventory stubs, 11 attachment stubs, 11 sound stubs, 15 link/prim/object stubs (`osForceCreateLink`, `osForceBreakLink`, `osDie`, `osMessageObject`, `osGetRezzingObject`, `osSetContentType`, `osVolumeDetect`, `osGetPrimCount`, `osGetSittingAvatarsCount`, etc.); tests verify defaults

## 12. Remaining stubs and miscellaneous (25 functions)

- [ ] 12.1 Register notecard stubs (verify existing 4 match C# signatures, add `osLoadedCreationDate`, `osLoadedCreationTime`, `osLoadedCreationID`), appearance/map stubs (`osOwnerSaveAppearance`, `osAgentSaveAppearance`, `osGetRegionMapTexture`), inertia stubs (6 functions), sit/stand stubs (10 functions), plus `osClearObjectAnimations`, `osResetAllScripts`, `osParticleSystem`, `osLinkParticleSystem`, `osInviteToGroup`, `osEjectFromGroup`, `osConsoleCommand`, `osRequestURL`, `osRequestSecureURL`, `osSetRot`, `osGetLinkColor`, `osKey2Name`, `osAvatarName2Key`, `osGetAvatarHomeURI`, `osListenRegex`; tests verify defaults

## 13. Port C# OSSL tests and catalog verification

- [ ] 13.1 Port `OSSL_ApiNpcTests.cs`, `OSSL_ApiAttachmentTests.cs`, and `OSSL_ApiAppearanceTest.cs` to Rust as stub-verification tests
- [ ] 13.2 Comprehensive catalog test: `function_catalog()` returns exactly 258 entries; spot-check threat levels match C# `CheckThreatLevel` calls

## 14. Second review (HIGH RISK: threat-level enforcement correctness)

- [ ] 14.1 Adversarial review: every function checks threat level, per-function overrides cannot be script-injected, INI parser rejects malformed input, no function registered without appearing in the catalog
- [ ] 14.2 Crypto review: AES key derivation matches C# `Rfc2898DeriveBytes` behaviour, no key material in error messages, constant-time comparison where needed
