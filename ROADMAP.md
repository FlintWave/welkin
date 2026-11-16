# Welkin Roadmap

OpenSim 0.9.3.0 C# to Rust port. LGPL-3.0-or-later.

## Current Release (v0.1.0-alpha)

Welkin builds, runs, and lets a viewer log in to a single-region grid on SQLite.
Both programs, `welkin-robust` and `welkin-region`, come from `cargo build
--release`. The workspace has 2,652 passing tests, 0 failures and 2 ignored.

What works in this release (see [viewer compatibility](docs/viewer-compatibility.md)
for which parts were checked in a live viewer and which only in code):

- Login, region handshake, movement, jumping and falling on real terrain, chat,
  instant messages, friends and profiles
- Objects: create, edit, link, take, return, rez from inventory and attach; saved
  and loaded again at start
- Terraforming with real terrain data, saved across restarts
- Rapier 3D physics, physical prims, script physics commands
- LSL scripts compiled to Luau and run in a sandboxed VM; about 60% of a sample
  of 25 real-world scripts compile. Chat, timer, event, sensor, control, sound
  and physics calls work at runtime
- Estate settings and object permissions, enforced by default, with a stable region UUID
- Groups: create, join, leave, invitations, roles and powers, titles, eject, group chat
- Sound, OAR and IAR archives, and account creation from the region console

Known limits of this release:

- Run on a trusted network only. The authentication and account service endpoints
  have no access-control gates yet (no `AllowSetPassword` check), and the portal
  handlers do not validate input.
- SQLite is the only database that has been run. MariaDB and PostgreSQL are untested
  and have known problems.
- The web portal is a scaffold: no binary mounts it and most handlers do nothing.
- Many LSL functions are registered in the VM but not connected to the running
  region and do nothing: `llEmail`, `llHTTPRequest`, `llHTTPResponse`,
  `llRequestURL`, `llRezObject`, `llRezAtRoot`, `llAttachToAvatar` and its detach
  call. `llRequestAgentData` returns placeholder data. Teleport between regions,
  NPC functions and dynamic textures are not wired either.
- The LSL compiler rejects some valid scripts, such as a float literal that
  starts with a dot (`.4`).
- Group notice attachments are not supported, and group proposals are out of scope. In group
  chat, the event for a second avatar joining a session can arrive late.
- Attachments are saved at logout, not on a timer, so a region crash loses
  attachment changes made since login.
- No 10-minute soak test and no recorded Aurora session are on record.
- PBR, Bakes on Mesh, Animesh, EEP day cycles, viewer search and media on a prim
  are not implemented.
- The grid's private port binds to `127.0.0.1`, so a region on another machine
  cannot reach it until you change the bind address.

Everything below this section is paused after the release unless it says
otherwise. Contributions are welcome; see [CONTRIBUTING.md](CONTRIBUTING.md).

## Known gaps from the 2026-11-15 audit

The audit of 2026-11-15 found dozens of systems that were built and unit-tested
but not connected to the running server. Rows marked RESOLVED were fixed and are
in the v0.1.0-alpha release. Every other row was not re-checked for this release:
treat it as open. A gap counts as closed only when a viewer action produces the
correct result, not when the code exists.

### P0: Core movement and physics [CRITICAL, world is unusable without these]

| Item | Status | What exists | What's broken |
|---|---|---|---|
| Avatar physics (AvatarController) | RESOLVED for standing, jump and fall (avatar_move.rs); the PID controller is not used | Full PID controller, gravity, terrain collision, jump | step() never called; main loop uses kinematic pos += vel * dt with z=22 clamp |
| Jump | RESOLVED (429699c, da62213, fac8b53) | UP_POS handling in AvatarController | Flag only processed when flying; ignored when grounded |
| Fly mode | partially wired | FLY flag detected, speed/animation set | State not latched; recomputed per-packet instead of toggled |
| PhysicsCommand channel | RESOLVED (753ed12); llSetHoverHeight and runtime STATUS_PHYSICS changes are rejected with a logged reason | mpsc channel type defined | Sender set to None; all LSL physics functions silently drop commands |
| Collision detection (avatar-prim) | partially wired | Rapier backend, events posted to scripts | Events fire but num_detected always 1, no llDetectedKey/Name/Pos data populated |
| Sensor scan | built, stub | llSensor/llSensorRepeat registered | Always returns num_detected: 0; never queries scene |
| Permission enforcement | RESOLVED (69f0db4, 416fa77); on by default | DefaultPermissionsModule registered (main.rs:253) | Never consulted; any user can edit/delete/move/rename any object |
| Object persistence | RESOLVED (d6e1128, d89f143); objects, terrain and estate survive a restart | store_object() (simulation.rs:420), collect_for_backup() (scene.rs:424) | Neither called from main.rs; all objects lost on restart |
| Script-scene propagation | RESOLVED (58e0fef) | llSetPos/llSetColor/llSetScale/llSetTexture in ScriptContext | Changes stay in per-script context; never written to SceneObjectPart, never broadcast |
| Avatar name from login | RESOLVED (58e0fef) | ScenePresence::new() at main.rs:4117 | Hardcoded "Test"/"User"; real name from login response never extracted |
| Cross-region teleport | bug | EntityTransferModule::teleport() called | Ok() and Err() paths both send TeleportFailed unconditionally (main.rs:3277) |
| Sound (LSL) | RESOLVED (6718355 to 82193d6) | llPlaySound/llTriggerSound/llLoopSound compile; SoundCommand enum exists | No Lua function registered in VM; sound_sender always None; calling crashes script |
| AgentAnimation packet | missing handler | GesturesModule registered | No handler for AgentAnimation::ID; viewer-triggered gestures silently dropped |
| Offline IM storage | stub | OfflineMessageModule registered | Never consulted; cross-region IM silently dropped, "offline storage pending" logged |

### P1: Terrain [CRITICAL, world building impossible without these]

| Item | Status | What exists | What's broken |
|---|---|---|---|
| Terrain brush (ModifyLand) | RESOLVED (ab6637a, e309498, 854b115) | apply_brush() with raise/lower/smooth/flatten/noise/revert | No ModifyLand packet handler; brush code never called |
| Terrain DCT compressor | RESOLVED (82368da, 854b115); matches OpenSimulator golden vectors | encode_patch() / PatchEncoder | Server uses flat-terrain bit-packing hack; can't send real heightfield data |
| Terrain file I/O | built, not wired | RAW32, PNG import/export | Module declared in lib.rs, never imported by any runtime crate |

### P2: Object and scene integrity [CRITICAL, objects must persist and behave correctly]

| Item | Status | What exists | What's broken |
|---|---|---|---|
| Vehicle dynamics | built, not wired | Full sled/car/boat/airplane/balloon simulation | step() never called from main loop |
| Keyframe motion | built, not wired | Forward/reverse/loop/ping-pong animation | Never instantiated; XML deserializer skips KeyframeMotion elements |
| Terse update throttling | built, not wired | Bandwidth-saving update suppression | Every update broadcasts to all clients unconditionally |
| Object persistence | RESOLVED (d6e1128, d89f143); objects, terrain and estate survive a restart | store_object/collect_for_backup exist, never called | Prims do NOT survive restart; scene is purely in-memory (see P0) |
| LSL physics behaviors | RESOLVED except llSetHoverHeight (753ed12) | llMoveToTarget, llSetHoverHeight, llRotLookAt | physics_sender channel is None; commands dropped |
| llTakeControls/llReleaseControls | built, not wired | Control registrations tracked in HashMap | AgentUpdate never dispatches control events to scripts |

### P3: Server infrastructure [HIGH, affects stability and operations]

| Item | Status | What exists | What's broken |
|---|---|---|---|
| Graceful shutdown | built, not wired | GracefulShutdown orchestrator with state flushing | main loop calls scene.shutdown() directly; no countdown or announcement |
| RegionModule tick/update | design gap | 48 modules registered | Trait has no tick() method; no module can do periodic work |
| PresenceAdded/Removed events | partially wired | Event channels exist | Only fire for NPCs, not real user login/logout |
| Permission checks | RESOLVED (416fa77) | DefaultPermissionsModule registered, PermissionCheckEvent defined | Zero enforcement on any object mutation; any user edits anything (see P0) |
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

## In Progress

| Change | Done | Remaining | Focus |
|---|---|---|---|
| integration-wiring | 30/35 | 5 | End-to-end integration verification |
| firestorm-session | 29/36 | 7 | Firestorm viewer compatibility testing |

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

## Paused: contributions welcome

None of these is started. The task counts come from each change's `tasks.md`.

| Change | Tasks | Summary |
|---|---|---|
| realmwalker | 0/43 ticked | Rust-native bot SDK (code for phases 1 and 2 exists with 86 unit tests; the checklist was reset) |
| script-testing | 0/18 | LSL corpus testing framework (phases 1 and 2 code committed) |
| modern-avatar-features | 0/25 | Modern avatar rendering, materials, PBR |
| eep-environment | 0/42 | Extended Environment (EEP) day cycles |
| llm-world-copilot | - | LLM-powered in-world assistant (design phase) |
| plugin-api | - | Plugin and extension API framework (design phase) |

## Future (paused: contributions welcome)

- Smooth region crossing: predictive handshake, dual presence
- A working web portal and first-run setup
- Group notice attachments
- MariaDB and PostgreSQL support, verified
- Welkin project website
- Ink NPC dialogue system (Python or Rust, no LLMs)
- Benchmark comparison against OpenSimulator

## Test Summary

- **2,652 tests passing**, 0 failures, 2 ignored (`cargo test --workspace`)
- 25 crates in the workspace, plus the Tauri companion app, which builds separately
- Both `welkin-region` and `welkin-robust` build and start

## Key Milestones

- **195/195**: phases 1 to 3 (foundation through parity) complete
- **Post-195**: OSSL, builtins, events and extension APIs shipped
- **2026-11-15 audit**: three-pass audit found 60 or more disconnected systems; the
  rows marked RESOLVED above were fixed afterwards
- **LSL compiler**: 69/69 tasks complete
- **v0.1.0-alpha**: first release; see Current Release above
