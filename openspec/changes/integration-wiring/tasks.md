# Integration Wiring Tasks

## Phase 1: Fix the broken paths

- [x] Remove ChatFromViewer stub in handlers.rs that masks the real chat implementation in main.rs; verify chat works between two avatars
- [x] Fix MapBlockRequest to query ROBUST GridService and return populated MapBlock replies with region names, coordinates, and access flags
- [x] Set PhysicsEnabled=true in welkin.toml; verify Rapier3D physics loop starts and objects respond to gravity
- [x] Replace flat 21m terrain with proper heightmap loading from terrain files; verify terrain renders with elevation in viewer
- [ ] Integration test: connect viewer, verify chat sends/receives, minimap shows data, dropped objects fall

## Phase 2: Script lifecycle

- [x] Verify UpdateScriptAgent cap triggers LSL compilation via welkin-script-compiler and returns success/error LLSD
- [x] Wire RezScript: when a prim with a script in task inventory is rezzed, start the script's state_entry
- [x] Wire ScriptContext channels: llSay/llShout/llWhisper output reaches ChatModule; llListen receives matching chat
- [x] Wire timer events: llSetTimerEvent schedules repeating timer callbacks to the script VM
- [x] Wire sensor events: llSensor/llSensorRepeat trigger sensor_event with detected objects/agents
- [x] Wire touch events: viewer touch packets (ObjectGrab, ObjectGrabUpdate, ObjectDeGrab) trigger touch_start/touch/touch_end in scripts
- [x] Wire task inventory access: llGetInventoryName, llGetInventoryNumber, llGetInventoryType read from the prim's inventory
- [x] Wire prim rez triggers script start: when a prim is created via llRezObject or viewer, scripts in its inventory auto-start
- [x] Wire script state persistence: save script state (globals, running/stopped) to DB on region shutdown; restore on restart
- [x] Wire control event forwarding: scripts that called llTakeControls receive control() events from AGENT_CONTROL flags
- [ ] Integration test: upload a script via viewer, verify it runs, says to chat, responds to touch, persists across region restart

## Phase 3: LSL completion

- [x] Implement llEmail, llGetNextEmail with SMTP send (lettre) and email queue polling
- [x] Implement llHTTPRequest, llSetHTTPBody with outbound HTTP, throttling, URL filtering
- [x] Implement llRequestURL, llRequestSecureURL, llReleaseURL for inbound HTTP to scripts
- [x] Implement llRezObject, llRezAtRoot: rez from script inventory at position with velocity and start param
- [x] Implement llSetLinkPrimitiveParams, llSetLinkPrimitiveParamsFast: modify linked prim properties
- [x] Implement llAttachToAvatar, llDetachFromAvatar, llAttachToAvatarTemp: attachment control via script
- [x] Implement llSetVehicleType and all llSetVehicle* functions: vehicle dynamics with 40+ VEHICLE_* constants from OpenSim Scene.cs
- [x] Implement llTarget, llTargetRemove: position target events (at_target/not_at_target)
- [x] Implement llRotTarget, llRotTargetRemove: rotation target events
- [x] Implement llRequestAgentData: query agent info (online status, birth date, payment info) via ROBUST
- [x] Implement llGetNotecardLine, llGetNumberOfNotecardLines: async notecard reading (dataserver event)

## Phase 4: Validation

- [x] Full test suite green (2,055 tests, zero regressions)
- [x] Connect Aurora viewer; verify login, chat, map, physics, scripts all function
- [ ] Connect Firestorm viewer; verify same feature set works
- [x] 10-minute soak test with no crashes or disconnects
- [ ] Test local teleport between regions
- [x] Verify graceful disconnect and presence cleanup
- [x] Test reconnection after viewer restart
- [ ] Document working/broken matrix for both viewers
