# Remaining Parity — Proposal

## Problem

After the LSL compiler, object interaction, viewer messaging, module activation,
and currency work packages, several categories of gaps remain before Welkin
reaches full OSgrid parity:

1. **11 deferred LSL builtins** requiring external infrastructure (HTTP client,
   SMTP, vehicle physics, PrimitiveParams, attachment control)
2. **~20 missing HTTP capabilities** the viewer expects
3. **Estate admin packets** (FreezeUser, EjectUser, EstateOwnerMessage)
4. **Cross-region teleport** (EntityTransferModule exists but not wired)
5. **Physics step verification** (Rapier code exists but unclear if stepping)
6. **~141 os* stubs** that log but don't do real work

## Scope

Address all remaining P1/P2 gaps in a single change with phased commits.

### Phase 1: Infrastructure builtins (the 11 deferred LSL tasks)
- llHTTPRequest/llSetHTTPBody — outbound HTTP with throttling
- llRequestURL/llRequestSecureURL/llReleaseURL — inbound HTTP cap URLs
- llEmail/llGetNextEmail — SMTP via lettre
- llSetPrimitiveParams/llSetLinkPrimitiveParams — all BUILD_* constants
- llSetVehicleType + VEHICLE_* constants — Rapier vehicle integration
- llRezObject/llRezAtRoot — rez from script inventory
- llAttachToAvatar/llDetachFromAvatar — attachment control
- llTarget/llRotTarget — position/rotation monitoring events
- llGetNotecardLine/llGetNumberOfNotecardLines — async notecard reading
- llRequestAgentData — ROBUST user query

### Phase 2: Missing HTTP capabilities (~20 caps)
- AvatarPickerSearch, ChatSessionRequest, CopyInventoryFromNotecard
- DispatchRegionInfo, EstateChangeInfo, EnvironmentSettings
- GroupMemberData, LandResources, MeshUploadFlag
- RemoteParcelRequest, SearchStatRequest, SetDisplayName
- SimConsoleAsync, UpdateGestureAgentInventory, UpdateGestureTaskInventory
- UpdateNotecardTaskInventory, UpdateScriptTask, ViewerMetrics
- ServerReleaseNotes

### Phase 3: Estate admin and teleport
- EstateOwnerMessage packet handler (dispatches ~20 sub-commands)
- FreezeUser, EjectUser, GodUpdateRegionInfo packets
- RequestRegionInfo packet
- Wire EntityTransferModule for cross-region teleport
- Wire HG modules for hypergrid teleport (already loaded in module-activation)

### Phase 4: Physics and os* completion
- Verify physics.step() runs in region main loop, add if missing
- Verify collision callbacks fire → script events
- Replace ~141 os* stubs with real implementations (priority: osNpc*,
  osTeleportAgent, osSetDynamicTextureURL, osGetSimulatorMemory, etc.)

### Out of scope
- New features not in OpenSim 0.9.3.0
- LSL preprocessor
- Experience tools API
