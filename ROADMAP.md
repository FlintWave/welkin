# Welkin Roadmap

OpenSim 0.9.3.0 C# to Rust port. LGPL-3.0-or-later.

## BLOCKING: Dead Systems Wiring (must complete before any new features)

Comprehensive audit (2026-11-15) found dozens of systems that are built,
unit-tested, and completely disconnected from the running server. Nothing
new gets built until every item here is wired in and verified by runtime
behavior (viewer action produces correct result), not code existence.

### P0: Core movement and physics [CRITICAL, world is unusable without these]

| Item | Status | What exists | What's broken |
|---|---|---|---|
| Avatar physics (AvatarController) | built, not wired | Full PID controller, gravity, terrain collision, jump | step() never called; main loop uses kinematic pos += vel * dt with z=22 clamp |
| Jump | built, not wired | UP_POS handling in AvatarController | Flag only processed when flying; ignored when grounded |
| Fly mode | partially wired | FLY flag detected, speed/animation set | State not latched; recomputed per-packet instead of toggled |
| PhysicsCommand channel | built, not wired | mpsc channel type defined | Sender set to None; all LSL physics functions silently drop commands |
| Collision detection (avatar-prim) | partially wired | Rapier backend, events posted to scripts | Events fire but num_detected always 1, no llDetectedKey/Name/Pos data populated |
| Sensor scan | built, stub | llSensor/llSensorRepeat registered | Always returns num_detected: 0; never queries scene |
| Permission enforcement | built, not wired | DefaultPermissionsModule registered (main.rs:253) | Never consulted; any user can edit/delete/move/rename any object |
| Object persistence | built, not wired | store_object() (simulation.rs:420), collect_for_backup() (scene.rs:424) | Neither called from main.rs; all objects lost on restart |
| Script-scene propagation | design gap | llSetPos/llSetColor/llSetScale/llSetTexture in ScriptContext | Changes stay in per-script context; never written to SceneObjectPart, never broadcast |
| Avatar name from login | bug | ScenePresence::new() at main.rs:4117 | Hardcoded "Test"/"User"; real name from login response never extracted |
| Cross-region teleport | bug | EntityTransferModule::teleport() called | Ok() and Err() paths both send TeleportFailed unconditionally (main.rs:3277) |
| Sound (LSL) | built, not wired | llPlaySound/llTriggerSound/llLoopSound compile; SoundCommand enum exists | No Lua function registered in VM; sound_sender always None; calling crashes script |
| AgentAnimation packet | missing handler | GesturesModule registered | No handler for AgentAnimation::ID; viewer-triggered gestures silently dropped |
| Offline IM storage | stub | OfflineMessageModule registered | Never consulted; cross-region IM silently dropped, "offline storage pending" logged |

### P1: Terrain [CRITICAL, world building impossible without these]

| Item | Status | What exists | What's broken |
|---|---|---|---|
| Terrain brush (ModifyLand) | built, not wired | apply_brush() with raise/lower/smooth/flatten/noise/revert | No ModifyLand packet handler; brush code never called |
| Terrain DCT compressor | built, not wired | encode_patch() / PatchEncoder | Server uses flat-terrain bit-packing hack; can't send real heightfield data |
| Terrain file I/O | built, not wired | RAW32, PNG import/export | Module declared in lib.rs, never imported by any runtime crate |

### P2: Object and scene integrity [CRITICAL, objects must persist and behave correctly]

| Item | Status | What exists | What's broken |
|---|---|---|---|
| Vehicle dynamics | built, not wired | Full sled/car/boat/airplane/balloon simulation | step() never called from main loop |
| Keyframe motion | built, not wired | Forward/reverse/loop/ping-pong animation | Never instantiated; XML deserializer skips KeyframeMotion elements |
| Terse update throttling | built, not wired | Bandwidth-saving update suppression | Every update broadcasts to all clients unconditionally |
| Object persistence | CONFIRMED NOT WIRED | store_object/collect_for_backup exist, never called | Prims do NOT survive restart; scene is purely in-memory (see P0) |
| LSL physics behaviors | built, not wired | llMoveToTarget, llSetHoverHeight, llRotLookAt | physics_sender channel is None; commands dropped |
| llTakeControls/llReleaseControls | built, not wired | Control registrations tracked in HashMap | AgentUpdate never dispatches control events to scripts |

### P3: Server infrastructure [HIGH, affects stability and operations]

| Item | Status | What exists | What's broken |
|---|---|---|---|
| Graceful shutdown | built, not wired | GracefulShutdown orchestrator with state flushing | main loop calls scene.shutdown() directly; no countdown or announcement |
| RegionModule tick/update | design gap | 48 modules registered | Trait has no tick() method; no module can do periodic work |
| PresenceAdded/Removed events | partially wired | Event channels exist | Only fire for NPCs, not real user login/logout |
| Permission checks | CONFIRMED NOT WIRED | DefaultPermissionsModule registered, PermissionCheckEvent defined | Zero enforcement on any object mutation; any user edits anything (see P0) |
| MoneyReceived event | built, not wired | Event channel subscribed | Never sent from any code path |
| LandChange event | built, not wired | Event channel subscribed | No parcel system fires it |

### P4: Stub handlers faking success [HIGH, these lie to the viewer]

| Item | Behavior | Correct behavior |
|---|---|---|
| AgentSit | Logs "confirmed sit", does nothing | Update presence to seated, parent to sit target, stop movement |
| ObjectBuy | Sends fake MoneyBalanceReply | Transfer ownership, debit/credit accounts |
| HomeLocation POST | Returns success, never persists | Store home location in user profile |
| NewFileAgentInventory | Returns random UUIDs | Store uploaded asset, create inventory item |
| UpdateNotecardAgentInventory | Returns random UUID | Store notecard content |
| GetDisplayNames | Returns "User Resident" for every UUID | Look up actual display name |
| SetDisplayName | Logs, doesn't persist | Store display name change |
| AgentPreferences | Returns canned defaults | Store and retrieve per-user preferences |
| EstateOwnerMessage (all sub-methods) | Log without acting | Apply estate setting changes |
| EnvironmentSettings GET/POST | Hardcoded default / log-and-ack | Store and serve actual day cycle data |
| LandResources | Returns hardcoded 15000/0 | Query actual prim counts |
| AvatarPickerSearch | Returns empty array | Search user database |
| RemoteParcelRequest | Returns nil/Unknown | Look up actual parcel data |
| CopyInventoryFromNotecard | Returns random UUID | Actually copy items |
| SimulatorFeatures | Hardcoded feature flags | Read actual server config |
| Gesture/Script task inventory updates | Return random UUIDs | Actually store updated content |

### P5: Missing packet handlers [MEDIUM, features don't exist yet]

| Packet group | Packets needed |
|---|---|
| Parcel system | ParcelPropertiesRequest, ParcelInfoRequest, ParcelDwellRequest, ParcelAccessListRequest/Update |
| Asset transfer | TransferRequest, AssetUploadRequest, SendXferPacket, ConfirmXferPacket |
| Mute list | MuteListRequest, UpdateMuteListEntry, RemoveMuteListEntry |
| Search/directory | DirFindQuery, DirPlacesQuery, DirLandQuery, DirClassifiedQuery, DirEventsQuery |
| Groups | GroupDataRequest, GroupRoleDataRequest, GroupMembersRequest, Create/Join/LeaveGroup |
| Map | MapItemRequest, MapNameRequest |
| Object operations | ObjectImage, ObjectRotation, ObjectFlagUpdate, ObjectClickAction, ObjectIncludeInSearch |
| GenericMessage | Used by many viewer features as catch-all |
| RegionHandshakeReply | Should trigger final login steps |
| AgentPause/Resume | Viewer minimized/restored state |
| RequestMultipleObjects | Bulk object data fetch |

### P6: Dead cap routes [LOW, code exists but viewer never told about them]

19 cap handlers are coded and routed but not listed in KNOWN_CAPS, so the
viewer never requests them: AvatarPickerSearch, ChatSessionRequest,
CopyInventoryFromNotecard, DispatchRegionInfo, EnvironmentSettings,
EstateChangeInfo, GroupMemberData, LandResources, MeshUploadFlag,
RemoteParcelRequest, SearchStatRequest, ServerReleaseNotes, SetDisplayName,
SimConsoleAsync, UpdateGestureAgentInventory, UpdateGestureTaskInventory,
UpdateNotecardTaskInventory, UpdateScriptTask, ViewerMetrics.

Fix: add these to the KNOWN_CAPS list in main.rs seed handler (~line 4437).
Many still need their stub handlers replaced with real implementations (see P4).

## In Progress (3 changes)

| Change | Done | Remaining | Focus |
|---|---|---|---|
| integration-wiring | 30/35 | 5 | End-to-end integration verification |
| firestorm-session | 29/36 | 7 | Firestorm viewer compatibility testing |
| realmwalker | 21/80 | 59 | Rust-native bot SDK |

## Done (24 changes, all tasks complete)

| Change | Tasks | Summary |
|---|---|---|
| port-foundation | 12/12 | Workspace scaffold, config, data layer, types |
| protocol-core | 15/15 | LLUDP message codec, packet dispatch, net server |
| grid-services | 14/14 | ROBUST services: login, grid, assets, inventory, auth |
| region-simulator-core | 34/34 | Region binary, scene graph, terrain, tick loop |
| prim-meshing | 19/19 | Prim serialization, texture entry, object updates |
| physics-backend | 19/19 | Rapier3D integration, heightfield terrain, collision |
| object-interaction | 24/24 | Object editing, manipulation, rez/derez, linking |
| viewer-messaging | 23/23 | IM handler, group messaging, identity/interaction |
| module-activation | 45/45 | 45 region modules activated (P0/P1/P2 tiers) |
| social-modules | 19/19 | Groups V2, friends, profiles, mute lists |
| region-modules | 33/33 | Chat, map, estate, land, NPC, wind, sound modules |
| script-engine | 18/18 | LSL parser, analyzer, Luau transpiler, VM, scheduler |
| hypergrid | 20/20 | Gatekeeper, user agent, HG friends/inventory/transfer |
| webrtc-voice | 11/11 | WebRTC voice module via wolfvoice sidecar |
| remaining-parity | 46/46 | Builtins, caps, handlers, estate admin, teleport |
| ossl-api | 18/18 | 110+ OSSL functions: NPC, agent, inventory, misc |
| currency-module | 23/23 | Currency contract, viewer protocol, LSL, external |
| cutover | 19/19 | Validation, migration tooling, deployment |
| external-grid-mode | 12/12 | OSGrid/HG-compatible external grid configuration |
| grid-portal | 27/27 | Web portal for grid management |
| open-source-audit | 12/12 | License compliance, dependency audit |
| capacity-assessor | 13/13 | Performance benchmarking and capacity planning |
| lsl-compiler | 69/69 | LSL parser, implicit coercion, corpus coverage |
| archives | 18/18 | OAR/IAR archive import and export |

## Not Started (4 changes)

| Change | Tasks | Summary |
|---|---|---|
| script-testing | 0/18 | LSL corpus testing framework (phases 1-2 code committed) |
| modern-avatar-features | 0/25 | Modern avatar rendering, materials, PBR |
| llm-world-copilot | - | LLM-powered in-world assistant (design phase) |
| plugin-api | - | Plugin/extension API framework (design phase) |

## Future (blocked until above is clean)

- EEP (Extended Environment Protocol): day cycles, per-parcel environments
- Smooth region crossing: predictive handshake, dual-presence
- Welkin web UI and project website
- Ink NPC dialogue system (Python/Rust, no LLMs)
- Benchmark comparison vs OpenSim

## Test Summary

- **2,355 tests passing**, 0 failures, 2 ignored
- 27 crates in workspace
- Both `welkin-region` and `welkin-robust` binaries build and start

## Key Milestones

- **195/195**: Phases 1-3 (foundation through parity) complete
- **Post-195**: OSSL, builtins, events, extension APIs shipped
- **2026-11-15 audit**: Three-pass audit (dead systems + deep sweep + adversarial trace) found 60+ disconnected systems
- **RealmWalker**: 21/80 tasks done (phases 1-2 code committed, 86 unit tests)
- **LSL compiler**: 69/69 tasks complete; corpus was 60% (15/25) before final parser fixes
- **Gray avatar fix**: b414fb9, FullObjectUpdate now carries baked textures
